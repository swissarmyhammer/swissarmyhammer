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
use crate::chain::{
    ChainContext, ChainLink, ChainResult, CTX_FILE_DIFFS, VALIDATOR_BLOCK_EXIT_CODE,
};
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
    /// Load changed files for Stop hooks.
    ///
    /// Returns the list of files that changed during the turn so the validator
    /// loader can match Stop-triggered RuleSets that have a `match.files`
    /// pattern (e.g. `code-quality` matches `*.rs`).
    ///
    /// Sources, in order of preference:
    /// 1. `turn_state.changed` — populated by `PostToolUseFileTracker` when a
    ///    pre-hash differs from the post-hash for a tool's tracked paths.
    /// 2. Sidecar diff filenames under `.avp/turn_diffs/<session_id>/` —
    ///    populated by the same tracker as `.diff` files keyed by encoded
    ///    file path.
    ///
    /// The sidecar fallback is what makes Stop hooks robust against the
    /// failure mode where `turn_state.changed` is empty even though diffs
    /// were written (e.g. process boundary issues, or any path where the
    /// state file is cleared but the diff sidecars are not). Without this
    /// fallback, the Stop chain silently rejects every ruleset with a
    /// `match.files` pattern — which was the regression captured by kanban
    /// task `01KQ8CXYMBGN1VTV4S89FGQYCA`.
    ///
    /// Returns `None` for non-Stop hooks, and `None` when neither source has
    /// any changed paths.
    fn load_changed_files_for_stop(&self, input: &I) -> Option<Vec<String>> {
        if input.hook_type() != HookType::Stop {
            return None;
        }
        let session_id = input.session_id();

        // Primary source: turn state's accumulated `changed` list.
        if let Ok(state) = self.turn_state.load(session_id) {
            if !state.changed.is_empty() {
                return Some(
                    state
                        .changed
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect(),
                );
            }
        }

        // Fallback: derive changed files from sidecar diff filenames. The
        // sidecars are the most authoritative on-disk record of what the
        // turn modified — if any are present, we treat their paths as the
        // changed-files list for Stop matching.
        let sidecar = self.turn_state.load_all_diffs(session_id);
        if sidecar.is_empty() {
            return None;
        }
        let mut files: Vec<String> = sidecar.into_keys().collect();
        files.sort();
        tracing::debug!(
            "ValidatorExecutorLink: Stop hook session={} changed_files derived from {} sidecar diff(s) (turn_state.changed was empty)",
            session_id,
            files.len()
        );
        Some(files)
    }

    /// Load accumulated diffs from sidecar files for Stop hooks.
    ///
    /// Reads all `.diff` files from `.avp/turn_diffs/<session_id>/` and converts
    /// them into `FileDiff` structs. Returns `None` if no diffs are found.
    fn load_diffs_from_sidecar(&self, input: &I) -> Option<Vec<crate::turn::FileDiff>> {
        let session_id = input.session_id();
        let all_diffs = self.turn_state.load_all_diffs(session_id);
        if all_diffs.is_empty() {
            return None;
        }

        let diffs: Vec<crate::turn::FileDiff> = all_diffs
            .into_iter()
            .map(|(path, diff_text)| {
                // Detect new files by checking for the standard unified diff marker
                let is_new_file = diff_text.contains("--- /dev/null");
                crate::turn::FileDiff {
                    path: std::path::PathBuf::from(&path),
                    diff_text,
                    is_new_file,
                    is_binary: false,
                }
            })
            .collect();

        tracing::debug!(
            "ValidatorExecutorLink: Loaded {} diffs from sidecar for session {}",
            diffs.len(),
            session_id
        );

        Some(diffs)
    }

    /// Collect diffs from the appropriate source and prepare context for validators.
    ///
    /// PostToolUse: diffs from ChainContext (set by PostToolUseFileTracker), prepared inline.
    /// Stop: diffs from sidecar files, passed raw for per-ruleset filtering in the runner.
    fn prepare_diffs(
        &self,
        input: &I,
        hook_type: HookType,
        ctx: &mut ChainContext,
        input_json: serde_json::Value,
    ) -> (serde_json::Value, Option<Vec<crate::turn::FileDiff>>) {
        let chain_diffs: Option<Vec<crate::turn::FileDiff>> = ctx.get(CTX_FILE_DIFFS);
        let sidecar_diffs = if chain_diffs.is_none() && hook_type == HookType::Stop {
            self.load_diffs_from_sidecar(input)
        } else {
            None
        };
        let effective_diffs = chain_diffs.as_deref().or(sidecar_diffs.as_deref());

        if hook_type == HookType::Stop {
            (input_json, effective_diffs.map(|d| d.to_vec()))
        } else {
            let prepared = crate::turn::prepare_validator_context(input_json, effective_diffs);
            (prepared, None)
        }
    }
}

/// Decide whether a hook type uses the stderr-only failure surface.
///
/// Stderr-only hooks have no structured stdout output channel for blocking,
/// so AVP signals a block by setting exit code [`VALIDATOR_BLOCK_EXIT_CODE`]
/// and writing the failure message to stderr. Stdout-style hooks (PreToolUse,
/// PostToolUse, Stop, etc.) signal a block by emitting JSON on stdout with
/// `decision: "block"` (or equivalent), so the exit code stays 0 for those.
fn hook_uses_stderr_only_failure_surface(hook_type: HookType) -> bool {
    matches!(
        hook_type,
        HookType::SessionStart
            | HookType::SessionEnd
            | HookType::Notification
            | HookType::SubagentStart
            | HookType::PreCompact
            | HookType::Setup
            | HookType::Elicitation
            | HookType::ElicitationResult
            | HookType::ConfigChange
            | HookType::WorktreeCreate
            | HookType::TeammateIdle
            | HookType::TaskCompleted
    )
}

/// Handle RuleSet results, returning appropriate ChainResult.
///
/// Walks all `ExecutedRuleSet`s and finds the first blocking failure (any
/// rule with `severity == Error` whose result is failed). When found:
/// - Returns [`ChainResult::Stop`] carrying a [`LinkOutput::from_validator_block`]
///   with the qualified rule name (`<ruleset>:<rule>`) and the blocking
///   rule's message.
/// - Sets [`VALIDATOR_BLOCK_EXIT_CODE`] on `ctx` for stderr-only hook types
///   (see [`hook_uses_stderr_only_failure_surface`]).
///
/// When no rule blocks, returns [`ChainResult::continue_empty`] and leaves
/// the context untouched. The output is agent-agnostic; agent strategies
/// transform it into their platform-specific format.
pub(super) fn handle_ruleset_results(
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

            if hook_uses_stderr_only_failure_surface(hook_type) {
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

/// Build a MatchContext from input implementing ValidatorMatchInfo.
fn build_match_context<I: ValidatorMatchInfo>(
    input: &I,
    changed_files: Option<Vec<String>>,
) -> MatchContext {
    let mut ctx = MatchContext::new(input.hook_type());
    if let Some(tool) = input.tool_name() {
        ctx = ctx.with_tool(tool);
    }
    if let Some(file) = input.file_path() {
        ctx = ctx.with_file(file);
    }
    if let Some(files) = changed_files {
        ctx = ctx.with_changed_files(files);
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
                self.common.session_id.as_deref().unwrap_or_default()
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
                self.common.session_id.as_deref().unwrap_or_default()
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

// New hook types with validator support (no tool fields)
impl_validator_match_info!(
    crate::strategy::claude::input::ElicitationInput,
    Elicitation
);
impl_validator_match_info!(
    crate::strategy::claude::input::ElicitationResultInput,
    ElicitationResult
);
impl_validator_match_info!(
    crate::strategy::claude::input::ConfigChangeInput,
    ConfigChange
);
impl_validator_match_info!(
    crate::strategy::claude::input::WorktreeCreateInput,
    WorktreeCreate
);
impl_validator_match_info!(
    crate::strategy::claude::input::TeammateIdleInput,
    TeammateIdle
);
impl_validator_match_info!(
    crate::strategy::claude::input::TaskCompletedInput,
    TaskCompleted
);

#[async_trait(?Send)]
impl<I> ChainLink<I> for ValidatorExecutorLink<I>
where
    I: crate::chain::link::HookInputType + ValidatorMatchInfo + serde::Serialize,
{
    async fn process(&self, input: &I, ctx: &mut ChainContext) -> ChainResult {
        let hook_type = input.hook_type();
        let changed_files = self.load_changed_files_for_stop(input);

        // For Stop hooks specifically, surface the resolved changed-files
        // count at info level. A Stop hook firing with zero changed files
        // means every RuleSet that has `match.files` patterns will be
        // rejected — which is the symptom reported in kanban task
        // `01KQ8CXYMBGN1VTV4S89FGQYCA`. Logging it here gives operators a
        // single line to grep when the Stop validator path goes silent.
        if hook_type == HookType::Stop {
            tracing::info!(
                changed_files_count = changed_files.as_ref().map_or(0, |f| f.len()),
                session_id = input.session_id(),
                "ValidatorExecutorLink: Stop hook resolved changed files",
            );
        }

        let match_ctx = build_match_context(input, changed_files.clone());
        let rulesets = self.loader.matching_rulesets(&match_ctx);

        if rulesets.is_empty() {
            tracing::trace!("ValidatorExecutorLink: No RuleSets for {:?}", hook_type);
            // For Stop hooks, also log at info level so the empty-match
            // case is visible without enabling trace output. PostToolUse
            // and friends fire often enough that info-level "no matches"
            // would be noise.
            if hook_type == HookType::Stop {
                tracing::info!("ValidatorExecutorLink: Stop hook matched 0 RuleSets",);
            }
            return ChainResult::continue_empty();
        }
        let matched_names: Vec<&str> = rulesets.iter().map(|rs| rs.name()).collect();
        tracing::info!(
            ruleset_count = rulesets.len(),
            rulesets = ?matched_names,
            "ValidatorExecutorLink: Executing {} RuleSets for {:?}",
            rulesets.len(),
            hook_type
        );

        let input_json = match serde_json::to_value(input) {
            Ok(json) => json,
            Err(e) => {
                tracing::warn!("ValidatorExecutorLink: Failed to serialize input: {}", e);
                return ChainResult::continue_empty();
            }
        };

        let (context_value, raw_diffs) = self.prepare_diffs(input, hook_type, ctx, input_json);

        let results = self
            .context
            .execute_rulesets(
                &rulesets,
                hook_type,
                &context_value,
                changed_files.as_deref(),
                raw_diffs.as_deref(),
            )
            .await;

        handle_ruleset_results(&results, hook_type, ctx)
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

    #[test]
    fn test_validator_match_info_post_tool_use() {
        let input: crate::types::PostToolUseInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "/src/main.rs"}
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::PostToolUse);
        assert_eq!(input.tool_name(), Some("Write"));
        assert_eq!(input.file_path(), Some("/src/main.rs"));
        assert_eq!(input.session_id(), "s");
    }

    #[test]
    fn test_validator_match_info_post_tool_use_failure() {
        let input: crate::types::PostToolUseFailureInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "PostToolUseFailure",
                "tool_name": "Bash",
                "tool_input": {"command": "ls"}
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::PostToolUseFailure);
        assert_eq!(input.tool_name(), Some("Bash"));
        assert_eq!(input.session_id(), "s");
    }

    #[test]
    fn test_validator_match_info_permission_request() {
        let input: crate::types::PermissionRequestInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "PermissionRequest",
                "tool_name": "Bash",
                "tool_input": {"command": "rm -rf /"}
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::PermissionRequest);
        assert_eq!(input.tool_name(), Some("Bash"));
    }

    #[test]
    fn test_validator_match_info_session_start() {
        let input: crate::types::SessionStartInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "SessionStart"
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::SessionStart);
        assert_eq!(input.tool_name(), None);
        assert_eq!(input.file_path(), None);
    }

    #[test]
    fn test_validator_match_info_session_end() {
        let input: crate::types::SessionEndInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "SessionEnd"
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::SessionEnd);
        assert_eq!(input.tool_name(), None);
    }

    #[test]
    fn test_validator_match_info_user_prompt_submit() {
        let input: crate::types::UserPromptSubmitInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "UserPromptSubmit",
                "prompt": "hello"
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::UserPromptSubmit);
        assert_eq!(input.tool_name(), None);
    }

    #[test]
    fn test_validator_match_info_notification() {
        let input: crate::types::NotificationInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "Notification"
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::Notification);
    }

    #[test]
    fn test_validator_match_info_subagent_start() {
        let input: crate::types::SubagentStartInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "SubagentStart"
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::SubagentStart);
    }

    #[test]
    fn test_validator_match_info_subagent_stop() {
        let input: crate::types::SubagentStopInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "SubagentStop"
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::SubagentStop);
    }

    #[test]
    fn test_validator_match_info_pre_compact() {
        let input: crate::types::PreCompactInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "PreCompact"
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::PreCompact);
    }

    #[test]
    fn test_validator_match_info_setup() {
        let input: crate::types::SetupInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "Setup"
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::Setup);
    }

    // New hook types with ValidatorMatchInfo

    #[test]
    fn test_validator_match_info_elicitation() {
        let input: crate::strategy::claude::input::ElicitationInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "Elicitation"
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::Elicitation);
        assert_eq!(input.tool_name(), None);
        assert_eq!(input.file_path(), None);
    }

    #[test]
    fn test_validator_match_info_elicitation_result() {
        let input: crate::strategy::claude::input::ElicitationResultInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "ElicitationResult"
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::ElicitationResult);
    }

    #[test]
    fn test_validator_match_info_config_change() {
        let input: crate::strategy::claude::input::ConfigChangeInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "ConfigChange"
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::ConfigChange);
    }

    #[test]
    fn test_validator_match_info_worktree_create() {
        let input: crate::strategy::claude::input::WorktreeCreateInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "WorktreeCreate"
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::WorktreeCreate);
    }

    #[test]
    fn test_validator_match_info_teammate_idle() {
        let input: crate::strategy::claude::input::TeammateIdleInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "TeammateIdle"
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::TeammateIdle);
    }

    #[test]
    fn test_validator_match_info_task_completed() {
        let input: crate::strategy::claude::input::TaskCompletedInput =
            serde_json::from_value(serde_json::json!({
                "session_id": "s",
                "transcript_path": "/p",
                "cwd": "/c",
                "permission_mode": "default",
                "hook_event_name": "TaskCompleted"
            }))
            .unwrap();

        assert_eq!(input.hook_type(), HookType::TaskCompleted);
    }

    #[test]
    fn test_validator_match_info_no_session_id() {
        // When session_id is None, session_id() should return ""
        let input: crate::types::StopInput = serde_json::from_value(serde_json::json!({
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "Stop"
        }))
        .unwrap();

        assert_eq!(input.session_id(), "");
    }

    #[test]
    fn test_build_match_context_with_tool() {
        let input: crate::types::PreToolUseInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "/src/lib.rs"}
        }))
        .unwrap();

        let ctx = build_match_context(&input, None);
        assert_eq!(ctx.hook_type, HookType::PreToolUse);
        assert_eq!(ctx.tool_name.as_deref(), Some("Write"));
        assert_eq!(ctx.file_path.as_deref(), Some("/src/lib.rs"));
    }

    #[test]
    fn test_build_match_context_without_tool() {
        let input: crate::types::StopInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "Stop"
        }))
        .unwrap();

        let ctx = build_match_context(&input, None);
        assert_eq!(ctx.hook_type, HookType::Stop);
        assert!(ctx.tool_name.is_none());
        assert!(ctx.file_path.is_none());
        assert!(ctx.changed_files.is_none());
    }

    #[test]
    fn test_build_match_context_with_changed_files() {
        let input: crate::types::StopInput = serde_json::from_value(serde_json::json!({
            "session_id": "s",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "Stop"
        }))
        .unwrap();

        let files = vec!["foo.rs".to_string(), "bar.ts".to_string()];
        let ctx = build_match_context(&input, Some(files.clone()));
        assert_eq!(ctx.hook_type, HookType::Stop);
        assert_eq!(ctx.changed_files, Some(files));
    }

    // ========================================================================
    // handle_ruleset_results: error-severity failure path
    //
    // These tests cover the chain-link decision logic in isolation. They are
    // the unit half of the end-to-end coverage promised by kanban task
    // 01KQ7M20F27D0Z67H9XX0XQ4QZ — when a validator returns failed with
    // severity Error, the link must produce ChainResult::Stop carrying a
    // LinkOutput::from_validator_block whose validator name and message
    // reflect the offending rule. The integration half lives in
    // tests/recording_replay_integration.rs.
    // ========================================================================

    use crate::chain::ChainContext;
    use crate::chain::VALIDATOR_BLOCK_EXIT_CODE;
    use crate::validator::{ExecutedRuleSet, RuleResult, Severity, ValidatorResult};

    /// Build an [`ExecutedRuleSet`] with the given rule results. Test helper.
    fn make_ruleset(name: &str, rules: Vec<RuleResult>) -> ExecutedRuleSet {
        ExecutedRuleSet {
            ruleset_name: name.to_string(),
            rule_results: rules,
        }
    }

    /// Build a single error-severity failed [`RuleResult`]. Test helper.
    fn failed_error_rule(name: &str, message: &str) -> RuleResult {
        RuleResult {
            rule_name: name.to_string(),
            severity: Severity::Error,
            result: ValidatorResult::fail(message),
        }
    }

    /// Build a single error-severity passed [`RuleResult`]. Test helper.
    fn passing_error_rule(name: &str) -> RuleResult {
        RuleResult {
            rule_name: name.to_string(),
            severity: Severity::Error,
            result: ValidatorResult::pass("ok"),
        }
    }

    /// Single failed error-severity rule on a Stop hook produces a
    /// ChainResult::Stop with the qualified rule name and failure message.
    ///
    /// Stop is a stdout-style hook (it surfaces blocks via JSON
    /// `decision: "block"` on stdout, not exit code 2), so the context exit
    /// code is left at its default 0. The chain executor itself promotes the
    /// final exit code to `VALIDATOR_BLOCK_EXIT_CODE` when the chain output
    /// has `continue_execution: false` — that promotion is exercised in the
    /// integration test.
    #[test]
    fn handle_ruleset_results_single_error_failure_stop_hook_returns_stop() {
        let results = vec![make_ruleset(
            "code-quality",
            vec![failed_error_rule(
                "no-magic-numbers",
                "Found magic number 8675309 on line 12",
            )],
        )];
        let mut ctx = ChainContext::new();

        let chain_result = handle_ruleset_results(&results, HookType::Stop, &mut ctx);

        match chain_result {
            ChainResult::Stop(output) => {
                assert_eq!(output.continue_execution, Some(false));
                let block = output
                    .validator_block
                    .expect("Stop output should carry validator_block info");
                assert_eq!(block.validator_name, "code-quality:no-magic-numbers");
                assert_eq!(block.message, "Found magic number 8675309 on line 12");
                assert_eq!(block.hook_type, HookType::Stop);
                // stop_reason mirrors the validator message so agent strategies
                // can surface it without re-reaching into validator_block.
                assert_eq!(
                    output.stop_reason.as_deref(),
                    Some("Found magic number 8675309 on line 12")
                );
            }
            other => panic!("expected ChainResult::Stop, got {:?}", other),
        }

        // Stop is NOT a stderr-only hook; ctx.exit_code() must remain 0.
        // The chain executor sets the final exit code based on the chain
        // output's continue_execution flag, not on ctx.exit_code() for Stop.
        assert_eq!(ctx.exit_code(), 0);
    }

    /// Multiple failed rules across rulesets — first ruleset's first blocking
    /// failure wins, and the chain stops without inspecting later rulesets.
    /// This locks in the documented contract that
    /// `ExecutedRuleSet::blocking_failures()` returns failures in input order
    /// and the link only surfaces the first one.
    #[test]
    fn handle_ruleset_results_multiple_failures_returns_first_blocking() {
        let results = vec![
            make_ruleset(
                "security-rules",
                vec![failed_error_rule(
                    "no-secrets",
                    "Hard-coded API key in src/config.rs",
                )],
            ),
            make_ruleset(
                "code-quality",
                vec![failed_error_rule(
                    "no-magic-numbers",
                    "Magic numbers everywhere",
                )],
            ),
        ];
        let mut ctx = ChainContext::new();

        let chain_result = handle_ruleset_results(&results, HookType::Stop, &mut ctx);

        match chain_result {
            ChainResult::Stop(output) => {
                let block = output
                    .validator_block
                    .expect("validator_block should be set");
                // First ruleset's first blocking failure wins.
                assert_eq!(block.validator_name, "security-rules:no-secrets");
                assert_eq!(block.message, "Hard-coded API key in src/config.rs");
            }
            other => panic!("expected ChainResult::Stop, got {:?}", other),
        }
    }

    /// A failed rule alongside passing rules in the same RuleSet — the
    /// blocking failure is still surfaced. Order within a RuleSet doesn't
    /// hide a blocking rule behind passing ones.
    #[test]
    fn handle_ruleset_results_failed_rule_alongside_passing_rules() {
        let results = vec![make_ruleset(
            "code-quality",
            vec![
                passing_error_rule("function-length"),
                failed_error_rule("no-magic-numbers", "Found magic number 8675309 on line 12"),
                passing_error_rule("missing-docs"),
            ],
        )];
        let mut ctx = ChainContext::new();

        let chain_result = handle_ruleset_results(&results, HookType::Stop, &mut ctx);

        match chain_result {
            ChainResult::Stop(output) => {
                let block = output
                    .validator_block
                    .expect("blocking failure must produce validator_block");
                assert_eq!(block.validator_name, "code-quality:no-magic-numbers");
                assert_eq!(block.message, "Found magic number 8675309 on line 12");
            }
            other => panic!("expected ChainResult::Stop, got {:?}", other),
        }
    }

    /// All rules passed → chain continues with no output. No exit code is
    /// set, no validator block info is produced.
    #[test]
    fn handle_ruleset_results_all_pass_returns_continue_empty() {
        let results = vec![make_ruleset(
            "code-quality",
            vec![
                passing_error_rule("function-length"),
                passing_error_rule("no-magic-numbers"),
            ],
        )];
        let mut ctx = ChainContext::new();

        let chain_result = handle_ruleset_results(&results, HookType::Stop, &mut ctx);

        match chain_result {
            ChainResult::Continue(None) => {}
            other => panic!("expected ChainResult::Continue(None), got {:?}", other),
        }
        assert_eq!(ctx.exit_code(), 0);
    }

    /// Empty results (no rulesets executed) → chain continues. This is the
    /// "no validators matched" branch.
    #[test]
    fn handle_ruleset_results_empty_returns_continue() {
        let results: Vec<ExecutedRuleSet> = vec![];
        let mut ctx = ChainContext::new();

        let chain_result = handle_ruleset_results(&results, HookType::Stop, &mut ctx);

        match chain_result {
            ChainResult::Continue(None) => {}
            other => panic!("expected ChainResult::Continue(None), got {:?}", other),
        }
        assert_eq!(ctx.exit_code(), 0);
    }

    /// Warn-severity failures are NOT blocking. The link continues even
    /// though the rule's result is "failed" — only `severity == Error`
    /// failures stop the chain.
    #[test]
    fn handle_ruleset_results_warn_severity_failure_does_not_block() {
        let results = vec![make_ruleset(
            "code-quality",
            vec![RuleResult {
                rule_name: "function-length".to_string(),
                severity: Severity::Warn,
                result: ValidatorResult::fail("function is 51 lines"),
            }],
        )];
        let mut ctx = ChainContext::new();

        let chain_result = handle_ruleset_results(&results, HookType::Stop, &mut ctx);

        match chain_result {
            ChainResult::Continue(None) => {}
            other => panic!(
                "warn-severity failure must not stop the chain, got {:?}",
                other
            ),
        }
        assert_eq!(ctx.exit_code(), 0);
    }

    /// For stderr-only hooks (SessionStart, ConfigChange, etc.), the link
    /// MUST set the context exit code to `VALIDATOR_BLOCK_EXIT_CODE`. Those
    /// hooks have no stdout JSON channel for blocking, so the only signal
    /// to claude-code is exit code 2 + stderr message.
    #[test]
    fn handle_ruleset_results_session_start_sets_exit_code_to_validator_block() {
        let results = vec![make_ruleset(
            "code-quality",
            vec![failed_error_rule(
                "no-magic-numbers",
                "Found magic number 42",
            )],
        )];
        let mut ctx = ChainContext::new();

        let chain_result = handle_ruleset_results(&results, HookType::SessionStart, &mut ctx);

        match chain_result {
            ChainResult::Stop(output) => {
                let block = output
                    .validator_block
                    .expect("stderr-only hook block still carries validator_block");
                assert_eq!(block.hook_type, HookType::SessionStart);
            }
            other => panic!("expected ChainResult::Stop, got {:?}", other),
        }
        assert_eq!(
            ctx.exit_code(),
            VALIDATOR_BLOCK_EXIT_CODE,
            "stderr-only hook must set exit code to VALIDATOR_BLOCK_EXIT_CODE"
        );
    }

    /// Round-trip: every hook type in `hook_uses_stderr_only_failure_surface`
    /// actually flips the exit code. Belt-and-braces against drift between
    /// the helper and the call site.
    #[test]
    fn handle_ruleset_results_all_stderr_only_hooks_set_exit_code() {
        let stderr_only_hooks = [
            HookType::SessionStart,
            HookType::SessionEnd,
            HookType::Notification,
            HookType::SubagentStart,
            HookType::PreCompact,
            HookType::Setup,
            HookType::Elicitation,
            HookType::ElicitationResult,
            HookType::ConfigChange,
            HookType::WorktreeCreate,
            HookType::TeammateIdle,
            HookType::TaskCompleted,
        ];

        for hook in stderr_only_hooks {
            assert!(
                hook_uses_stderr_only_failure_surface(hook),
                "{:?} should be classified stderr-only",
                hook
            );

            let results = vec![make_ruleset(
                "code-quality",
                vec![failed_error_rule("no-magic-numbers", "boom")],
            )];
            let mut ctx = ChainContext::new();
            let _ = handle_ruleset_results(&results, hook, &mut ctx);
            assert_eq!(
                ctx.exit_code(),
                VALIDATOR_BLOCK_EXIT_CODE,
                "stderr-only hook {:?} did not set exit code",
                hook
            );
        }
    }

    /// Round-trip: stdout-style hooks must NOT set the exit code. Together
    /// with `..._all_stderr_only_hooks_set_exit_code` this pins down both
    /// halves of the matches! arm.
    #[test]
    fn handle_ruleset_results_stdout_style_hooks_leave_exit_code_default() {
        let stdout_style_hooks = [
            HookType::PreToolUse,
            HookType::PostToolUse,
            HookType::PostToolUseFailure,
            HookType::PermissionRequest,
            HookType::Stop,
            HookType::SubagentStop,
            HookType::UserPromptSubmit,
        ];

        for hook in stdout_style_hooks {
            assert!(
                !hook_uses_stderr_only_failure_surface(hook),
                "{:?} should NOT be classified stderr-only",
                hook
            );

            let results = vec![make_ruleset(
                "code-quality",
                vec![failed_error_rule("no-magic-numbers", "boom")],
            )];
            let mut ctx = ChainContext::new();
            let _ = handle_ruleset_results(&results, hook, &mut ctx);
            assert_eq!(
                ctx.exit_code(),
                0,
                "stdout-style hook {:?} should leave exit code at default 0",
                hook
            );
        }
    }
}
