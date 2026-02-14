//! Chain factory for creating default chains with appropriate links.
//!
//! This module provides a factory pattern for constructing chains that include
//! common links like file tracking and validator execution. Chains use
//! `ValidatorContextStarter` to prevent infinite recursion in subagents.

use std::sync::Arc;

use crate::context::AvpContext;
use crate::turn::TurnStateManager;
use crate::types::{
    PostToolUseInput, PreToolUseInput, SessionEndInput, SessionStartInput, StopInput,
};
use crate::validator::ValidatorLoader;

use super::executor::Chain;
use super::links::{
    PostToolUseFileTracker, PreToolUseFileTracker, StopCleanup, ValidatorExecutorLink,
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
    /// Note: We do NOT clear turn state on session start because subagents
    /// would clear the main session's tracked file changes. Turn state
    /// persists across sessions within a project.
    pub fn session_start_chain(&self) -> Chain<SessionStartInput> {
        Chain::new(ValidatorContextStarter::new()).add_link(self.validator_link())
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
    /// - StopCleanup: Clears turn state AFTER validators have seen the changes
    ///
    /// Note: Stop hook validators receive the list of changed files
    /// accumulated during the turn via the turn state. After validators run,
    /// the state is cleared for the next turn.
    ///
    /// Important: When CLAUDE_ACP is set (subagent context),
    /// we exit early in main.rs before any chain processing, so this
    /// cleanup won't incorrectly clear the main session's state.
    pub fn stop_chain(&self) -> Chain<StopInput> {
        Chain::new(ValidatorContextStarter::new())
            .add_link(self.validator_link())
            .add_link(StopCleanup::new(self.turn_state.clone()))
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

        // SessionStart chain only has ValidatorExecutor - no cleanup link
        // because subagents would clear the main session's turn state
        assert_eq!(chain.len(), 1);
        let names = chain.link_names();
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
    fn test_stop_chain_has_validator_and_cleanup_links() {
        let (_temp, factory) = create_test_factory();
        let chain = factory.stop_chain();

        // Stop chain has ValidatorExecutor followed by StopCleanup
        assert_eq!(chain.len(), 2);
        let names = chain.link_names();
        assert!(names.contains(&"ValidatorExecutor"));
        assert!(names.contains(&"StopCleanup"));
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
