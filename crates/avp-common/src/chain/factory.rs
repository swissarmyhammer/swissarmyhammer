//! Chain factory for creating default chains with appropriate links.
//!
//! This module provides a factory pattern for constructing chains that include
//! common links like file tracking and validator execution. Chains use
//! `ValidatorContextStarter` to prevent infinite recursion in subagents.

use std::sync::Arc;

use crate::context::AvpContext;
use crate::strategy::claude::input::{
    ConfigChangeInput, ElicitationInput, ElicitationResultInput, TaskCompletedInput,
    TeammateIdleInput, WorktreeCreateInput,
};
use crate::turn::TurnStateManager;
use crate::types::{
    PostToolUseInput, PreToolUseInput, SessionEndInput, SessionStartInput, StopInput,
};
use crate::validator::ValidatorLoader;

use super::executor::Chain;
use super::links::{
    PostToolUseFileTracker, PreToolUseFileTracker, SessionStartCleanup, ValidatorExecutorLink,
};
use super::starters::ValidatorContextStarter;

/// Factory for creating chains with default links.
///
/// The factory holds shared references to resources needed by chain links:
/// - AVP context for validator execution
/// - Validator loader for finding matching validators
/// - Turn state manager for file change tracking
pub struct ChainFactory {
    /// AVP context providing access to the agent and prompt library for validator execution.
    context: Arc<AvpContext>,
    /// Validator loader containing all loaded validators for matching against hook events.
    loader: Arc<ValidatorLoader>,
    /// Turn state manager for tracking file changes across PreToolUse/PostToolUse hooks.
    turn_state: Arc<TurnStateManager>,
}

impl ChainFactory {
    /// Create a new chain factory with the given resources.
    pub fn new(
        context: Arc<AvpContext>,
        loader: Arc<ValidatorLoader>,
        turn_state: Arc<TurnStateManager>,
    ) -> Self {
        Self {
            context,
            loader,
            turn_state,
        }
    }

    /// Helper: create a ValidatorExecutorLink with shared resources.
    fn validator_link<I>(&self) -> ValidatorExecutorLink<I> {
        ValidatorExecutorLink::new(
            self.context.clone(),
            self.loader.clone(),
            self.turn_state.clone(),
        )
    }

    /// Create a chain for SessionStart hooks.
    ///
    /// The chain includes:
    /// - SessionStartCleanup: Clears turn state and sidecar diffs for this session
    /// - ValidatorExecutorLink: Runs matching validators
    ///
    /// SessionStart is the natural reset point for a fresh turn. Diffs from
    /// the previous turn survive Stop (for post-mortem debugging) and are
    /// cleaned here. Each session clears only its own scoped directory.
    pub fn session_start_chain(&self) -> Chain<SessionStartInput> {
        Chain::new(ValidatorContextStarter::new())
            .add_link(SessionStartCleanup::new(self.turn_state.clone()))
            .add_link(self.validator_link())
    }

    /// Create a chain for SessionEnd hooks.
    ///
    /// Note: We do NOT clear turn state on session end because subagents
    /// would clear the main session's tracked file changes.
    pub fn session_end_chain(&self) -> Chain<SessionEndInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Create a chain for PreToolUse hooks.
    ///
    /// The chain includes:
    /// - PreToolUseFileTracker: Hashes files before tool execution
    /// - ValidatorExecutorLink: Runs matching validators
    pub fn pre_tool_use_chain(&self) -> Chain<PreToolUseInput> {
        Chain::new(ValidatorContextStarter::new())
            .add_link(PreToolUseFileTracker::new(self.turn_state.clone()))
            .add_link(self.validator_link())
    }

    /// Create a chain for PostToolUse hooks.
    ///
    /// The chain includes:
    /// - PostToolUseFileTracker: Detects file changes after tool execution
    /// - ValidatorExecutorLink: Runs matching validators
    pub fn post_tool_use_chain(&self) -> Chain<PostToolUseInput> {
        Chain::new(ValidatorContextStarter::new())
            .add_link(PostToolUseFileTracker::new(self.turn_state.clone()))
            .add_link(self.validator_link())
    }

    /// Create a chain for Stop hooks.
    ///
    /// The chain includes:
    /// - ValidatorExecutorLink: Runs matching validators (with changed files)
    ///
    /// Note: Stop hook validators receive the list of changed files
    /// accumulated during the turn via the turn state. State is NOT cleared
    /// here -- it survives past Stop for post-mortem debugging and is cleaned
    /// at SessionStart instead (by `SessionStartCleanup`).
    pub fn stop_chain(&self) -> Chain<StopInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Create a chain for Elicitation hooks.
    ///
    /// Elicitation requests user input on behalf of an MCP server. Validators
    /// can inspect or block elicitation requests.
    pub fn elicitation_chain(&self) -> Chain<ElicitationInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Create a chain for ElicitationResult hooks.
    ///
    /// ElicitationResult fires when the user responds to an MCP elicitation.
    /// Validators can inspect or block based on the user's response.
    pub fn elicitation_result_chain(&self) -> Chain<ElicitationResultInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Create a chain for ConfigChange hooks.
    ///
    /// ConfigChange fires when user or project configuration files change.
    /// Validators can inspect or block configuration changes.
    pub fn config_change_chain(&self) -> Chain<ConfigChangeInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Create a chain for WorktreeCreate hooks.
    ///
    /// WorktreeCreate fires when a git worktree is created. Validators can
    /// inspect or block worktree creation based on branch names or paths.
    pub fn worktree_create_chain(&self) -> Chain<WorktreeCreateInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Create a chain for TeammateIdle hooks.
    ///
    /// TeammateIdle fires when an agent teammate goes idle. Validators can
    /// inspect or block based on which teammate became idle.
    pub fn teammate_idle_chain(&self) -> Chain<TeammateIdleInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Create a chain for TaskCompleted hooks.
    ///
    /// TaskCompleted fires when a task is marked complete. Validators can
    /// inspect or block based on task identity.
    pub fn task_completed_chain(&self) -> Chain<TaskCompletedInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
    }

    /// Get the turn state manager for external access.
    pub fn turn_state(&self) -> &Arc<TurnStateManager> {
        &self.turn_state
    }

    /// Get the validator loader for external access.
    pub fn validator_loader(&self) -> &Arc<ValidatorLoader> {
        &self.loader
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::TempDir;

    fn create_test_factory() -> (TempDir, ChainFactory) {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".git")).unwrap();

        // Change to temp dir
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        // Disable agent execution
        std::env::set_var("AVP_SKIP_AGENT", "1");

        let context = Arc::new(AvpContext::init().unwrap());
        let loader = Arc::new(ValidatorLoader::new());
        let turn_state = Arc::new(TurnStateManager::new(temp.path()));

        std::env::set_current_dir(original_dir).unwrap();

        (temp, ChainFactory::new(context, loader, turn_state))
    }

    #[test]
    #[serial(cwd)]
    fn test_session_start_chain_has_links() {
        let (_temp, factory) = create_test_factory();
        let chain = factory.session_start_chain();

        // SessionStart chain has cleanup (turn state + diffs) + ValidatorExecutor
        assert_eq!(chain.len(), 2);
        let names = chain.link_names();
        assert!(names.contains(&"SessionStartCleanup"));
        assert!(names.contains(&"ValidatorExecutor"));
    }

    #[test]
    #[serial(cwd)]
    fn test_pre_tool_use_chain_has_links() {
        let (_temp, factory) = create_test_factory();
        let chain = factory.pre_tool_use_chain();

        assert_eq!(chain.len(), 2);
        let names = chain.link_names();
        assert!(names.contains(&"PreToolUseFileTracker"));
        assert!(names.contains(&"ValidatorExecutor"));
    }

    #[test]
    #[serial(cwd)]
    fn test_post_tool_use_chain_has_links() {
        let (_temp, factory) = create_test_factory();
        let chain = factory.post_tool_use_chain();

        assert_eq!(chain.len(), 2);
        let names = chain.link_names();
        assert!(names.contains(&"PostToolUseFileTracker"));
        assert!(names.contains(&"ValidatorExecutor"));
    }

    #[test]
    #[serial(cwd)]
    fn test_stop_chain_has_validator_only() {
        let (_temp, factory) = create_test_factory();
        let chain = factory.stop_chain();

        // Stop chain has only ValidatorExecutor (no cleanup -- state survives for debugging)
        assert_eq!(chain.len(), 1);
        let names = chain.link_names();
        assert!(names.contains(&"ValidatorExecutor"));
    }

    #[tokio::test]
    #[serial(cwd)]
    async fn test_session_start_chain_executes() {
        let (_temp, factory) = create_test_factory();
        let mut chain = factory.session_start_chain();

        let input: SessionStartInput = serde_json::from_value(serde_json::json!({
            "session_id": "test-session",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "SessionStart"
        }))
        .unwrap();

        let (output, exit_code) = chain.execute(&input).await.unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[test]
    #[serial(cwd)]
    fn test_session_end_chain_has_links() {
        let (_temp, factory) = create_test_factory();
        let chain = factory.session_end_chain();

        // SessionEnd chain has ValidatorExecutor
        assert_eq!(chain.len(), 1);
        let names = chain.link_names();
        assert!(names.contains(&"ValidatorExecutor"));
    }

    #[test]
    #[serial(cwd)]
    fn test_turn_state_getter() {
        let (_temp, factory) = create_test_factory();
        let turn_state = factory.turn_state();

        // Verify we can access the turn state manager
        // Load returns empty state for nonexistent sessions
        let state = turn_state.load("nonexistent").unwrap_or_default();
        assert!(state.changed_files_as_strings().is_empty());
    }

    #[test]
    #[serial(cwd)]
    fn test_validator_loader_getter() {
        let (_temp, factory) = create_test_factory();
        let loader = factory.validator_loader();

        // Verify we can access the validator loader
        // It should be empty since we created a new one
        assert!(loader.is_empty());
    }
}
