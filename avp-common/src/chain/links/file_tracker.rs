//! Chain links for tracking file changes during a turn.
//!
//! These links work together to detect which files actually changed:
//! - `PreToolUseFileTracker`: Extracts paths from tool input and hashes files before execution
//! - `PostToolUseFileTracker`: Hashes files after execution and records changes
//! - `SessionStartCleanup` / `SessionEndCleanup`: Clear turn state at session boundaries

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;

use crate::chain::{ChainContext, ChainLink, ChainResult};
use crate::turn::{extract_paths, hash_files, TurnStateManager};
use crate::types::{
    PostToolUseInput, PreToolUseInput, SessionEndInput, SessionStartInput, StopInput,
};

/// Macro to generate cleanup chain links that clear turn state.
///
/// This eliminates code duplication across SessionStartCleanup, SessionEndCleanup,
/// and StopCleanup which all have identical behavior: clear turn state for a session.
macro_rules! cleanup_chain_link {
    (
        $(#[$meta:meta])*
        $struct_name:ident,
        $input_type:ty,
        $link_name:literal,
        $log_msg:literal
    ) => {
        $(#[$meta])*
        pub struct $struct_name {
            turn_state: Arc<TurnStateManager>,
        }

        impl $struct_name {
            /// Create a new cleanup link.
            pub fn new(turn_state: Arc<TurnStateManager>) -> Self {
                Self { turn_state }
            }
        }

        #[async_trait(?Send)]
        impl ChainLink<$input_type> for $struct_name {
            async fn process(&self, input: &$input_type, _ctx: &mut ChainContext) -> ChainResult {
                tracing::debug!(
                    concat!($link_name, ": ", $log_msg, " {}"),
                    input.common.session_id
                );

                if let Err(e) = self.turn_state.clear(&input.common.session_id) {
                    tracing::warn!(concat!($link_name, ": Failed to clear turn state: {}"), e);
                }

                ChainResult::continue_empty()
            }

            fn name(&self) -> &'static str {
                $link_name
            }
        }
    };
}

/// Chain link that extracts file paths from tool input and hashes them before execution.
pub struct PreToolUseFileTracker {
    turn_state: Arc<TurnStateManager>,
}

impl PreToolUseFileTracker {
    /// Create a new PreToolUseFileTracker.
    pub fn new(turn_state: Arc<TurnStateManager>) -> Self {
        Self { turn_state }
    }
}

#[async_trait(?Send)]
impl ChainLink<PreToolUseInput> for PreToolUseFileTracker {
    async fn process(&self, input: &PreToolUseInput, _ctx: &mut ChainContext) -> ChainResult {
        // Extract paths from tool input
        let paths = extract_paths(&input.tool_input);

        if paths.is_empty() {
            tracing::trace!(
                "PreToolUseFileTracker: No paths found in {} tool input",
                input.tool_name
            );
            return ChainResult::continue_empty();
        }

        // Need a tool_use_id to track this
        let Some(tool_use_id) = &input.tool_use_id else {
            tracing::trace!(
                "PreToolUseFileTracker: No tool_use_id for {} tool, skipping",
                input.tool_name
            );
            return ChainResult::continue_empty();
        };

        tracing::debug!(
            "PreToolUseFileTracker: Hashing {} paths for tool {} ({})",
            paths.len(),
            input.tool_name,
            tool_use_id
        );

        // Hash files before tool execution
        let hashes = hash_files(&paths);

        // Store in turn state
        match self.turn_state.load(&input.common.session_id) {
            Ok(mut state) => {
                state.pending.insert(tool_use_id.clone(), hashes);
                if let Err(e) = self.turn_state.save(&input.common.session_id, &state) {
                    tracing::warn!("PreToolUseFileTracker: Failed to save turn state: {}", e);
                }
            }
            Err(e) => {
                tracing::warn!("PreToolUseFileTracker: Failed to load turn state: {}", e);
            }
        }

        ChainResult::continue_empty()
    }

    fn name(&self) -> &'static str {
        "PreToolUseFileTracker"
    }
}

/// Chain link that compares file hashes after tool execution to detect changes.
pub struct PostToolUseFileTracker {
    turn_state: Arc<TurnStateManager>,
}

impl PostToolUseFileTracker {
    /// Create a new PostToolUseFileTracker.
    pub fn new(turn_state: Arc<TurnStateManager>) -> Self {
        Self { turn_state }
    }
}

#[async_trait(?Send)]
impl ChainLink<PostToolUseInput> for PostToolUseFileTracker {
    async fn process(&self, input: &PostToolUseInput, _ctx: &mut ChainContext) -> ChainResult {
        let Some(tool_use_id) = &input.tool_use_id else {
            return ChainResult::continue_empty();
        };

        let mut state = match self.turn_state.load(&input.common.session_id) {
            Ok(state) => state,
            Err(e) => {
                tracing::warn!("PostToolUseFileTracker: Failed to load turn state: {}", e);
                return ChainResult::continue_empty();
            }
        };

        // Get pending hashes for this tool
        let Some(pre_hashes) = state.pending.remove(tool_use_id) else {
            tracing::trace!(
                "PostToolUseFileTracker: No pending hashes for tool {}",
                tool_use_id
            );
            return ChainResult::continue_empty();
        };

        // Compare hashes
        let mut changed_count = 0;
        for (path, pre_hash) in pre_hashes {
            let post_hash = crate::turn::hash_file(&path);

            if pre_hash != post_hash {
                tracing::debug!(
                    "PostToolUseFileTracker: File changed: {} (pre: {:?}, post: {:?})",
                    path.display(),
                    pre_hash,
                    post_hash
                );

                if !state.changed.contains(&path) {
                    state.changed.push(path);
                    changed_count += 1;
                }
            }
        }

        if changed_count > 0 {
            tracing::debug!(
                "PostToolUseFileTracker: {} file(s) changed by {} tool",
                changed_count,
                input.tool_name
            );
        }

        // Save updated state
        if let Err(e) = self.turn_state.save(&input.common.session_id, &state) {
            tracing::warn!("PostToolUseFileTracker: Failed to save turn state: {}", e);
        }

        ChainResult::continue_empty()
    }

    fn name(&self) -> &'static str {
        "PostToolUseFileTracker"
    }
}

cleanup_chain_link!(
    /// Chain link that clears turn state on session start.
    SessionStartCleanup,
    SessionStartInput,
    "SessionStartCleanup",
    "Clearing turn state for session"
);

cleanup_chain_link!(
    /// Chain link that clears turn state on session end.
    SessionEndCleanup,
    SessionEndInput,
    "SessionEndCleanup",
    "Clearing turn state for session"
);

cleanup_chain_link!(
    /// Chain link that clears turn state AFTER Stop validators have run.
    ///
    /// This is the proper cleanup point:
    /// 1. File changes accumulate during PreToolUse/PostToolUse
    /// 2. Stop hook fires, validators see all accumulated changes
    /// 3. This link clears state for the next turn
    ///
    /// We don't clear at SessionStart/SessionEnd because subagents would
    /// clear the main session's tracked changes.
    StopCleanup,
    StopInput,
    "StopCleanup",
    "Clearing turn state after Stop validators for session"
);

/// Load changed files for a session from turn state.
///
/// This is a utility function for use by the Stop hook handler.
pub fn load_changed_files(turn_state: &TurnStateManager, session_id: &str) -> Vec<PathBuf> {
    match turn_state.load(session_id) {
        Ok(state) => state.changed,
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
    use crate::types::CommonInput;
    use tempfile::TempDir;

    fn create_test_turn_state() -> (Arc<TurnStateManager>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let manager = Arc::new(TurnStateManager::new(temp_dir.path()));
        (manager, temp_dir)
    }

    fn create_pre_tool_use_input(
        session_id: &str,
        tool_name: &str,
        tool_use_id: &str,
        tool_input: serde_json::Value,
    ) -> PreToolUseInput {
        PreToolUseInput {
            common: CommonInput {
                session_id: session_id.to_string(),
                transcript_path: "/tmp/transcript.jsonl".to_string(),
                cwd: "/tmp".to_string(),
                permission_mode: "default".to_string(),
                hook_event_name: crate::types::HookType::PreToolUse,
            },
            tool_name: tool_name.to_string(),
            tool_input,
            tool_use_id: Some(tool_use_id.to_string()),
        }
    }

    fn create_post_tool_use_input(
        session_id: &str,
        tool_name: &str,
        tool_use_id: &str,
        tool_input: serde_json::Value,
    ) -> PostToolUseInput {
        PostToolUseInput {
            common: CommonInput {
                session_id: session_id.to_string(),
                transcript_path: "/tmp/transcript.jsonl".to_string(),
                cwd: "/tmp".to_string(),
                permission_mode: "default".to_string(),
                hook_event_name: crate::types::HookType::PostToolUse,
            },
            tool_name: tool_name.to_string(),
            tool_input,
            tool_result: None,
            tool_use_id: Some(tool_use_id.to_string()),
        }
    }

    #[tokio::test]
    async fn test_pre_tool_use_extracts_and_hashes() {
        let (turn_state, temp_dir) = create_test_turn_state();

        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "original content").unwrap();

        let tracker = PreToolUseFileTracker::new(turn_state.clone());
        let input = create_pre_tool_use_input(
            "session-1",
            "Edit",
            "tool-1",
            serde_json::json!({
                "file_path": test_file.to_string_lossy()
            }),
        );

        let mut ctx = ChainContext::new();
        let result = tracker.process(&input, &mut ctx).await;

        assert!(matches!(result, ChainResult::Continue(None)));

        // Check that pending hash was stored
        let state = turn_state.load("session-1").unwrap();
        assert!(state.pending.contains_key("tool-1"));
        let hashes = state.pending.get("tool-1").unwrap();
        assert!(hashes.contains_key(&test_file));
        assert!(hashes.get(&test_file).unwrap().is_some());
    }

    #[tokio::test]
    async fn test_post_tool_use_detects_change() {
        let (turn_state, temp_dir) = create_test_turn_state();

        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "original content").unwrap();

        // First, run PreToolUse
        let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
        let pre_input = create_pre_tool_use_input(
            "session-1",
            "Edit",
            "tool-1",
            serde_json::json!({
                "file_path": test_file.to_string_lossy()
            }),
        );
        let mut ctx = ChainContext::new();
        pre_tracker.process(&pre_input, &mut ctx).await;

        // Modify the file (simulating tool execution)
        std::fs::write(&test_file, "modified content").unwrap();

        // Run PostToolUse
        let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
        let post_input = create_post_tool_use_input(
            "session-1",
            "Edit",
            "tool-1",
            serde_json::json!({
                "file_path": test_file.to_string_lossy()
            }),
        );
        post_tracker.process(&post_input, &mut ctx).await;

        // Check that change was detected
        let state = turn_state.load("session-1").unwrap();
        assert!(state.changed.contains(&test_file));
        assert!(state.pending.is_empty()); // Pending should be cleared
    }

    #[tokio::test]
    async fn test_post_tool_use_no_change() {
        let (turn_state, temp_dir) = create_test_turn_state();

        // Create a test file
        let test_file = temp_dir.path().join("test.txt");
        std::fs::write(&test_file, "original content").unwrap();

        // Run PreToolUse
        let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
        let pre_input = create_pre_tool_use_input(
            "session-1",
            "Edit",
            "tool-1",
            serde_json::json!({
                "file_path": test_file.to_string_lossy()
            }),
        );
        let mut ctx = ChainContext::new();
        pre_tracker.process(&pre_input, &mut ctx).await;

        // Don't modify the file

        // Run PostToolUse
        let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
        let post_input = create_post_tool_use_input(
            "session-1",
            "Edit",
            "tool-1",
            serde_json::json!({
                "file_path": test_file.to_string_lossy()
            }),
        );
        post_tracker.process(&post_input, &mut ctx).await;

        // Check that no change was detected
        let state = turn_state.load("session-1").unwrap();
        assert!(state.changed.is_empty());
    }

    #[tokio::test]
    async fn test_session_start_clears_state() {
        let (turn_state, _temp_dir) = create_test_turn_state();

        // Set up some state
        let mut state = crate::turn::TurnState::new();
        state.changed.push(PathBuf::from("/some/file.txt"));
        turn_state.save("session-1", &state).unwrap();

        // Run SessionStartCleanup
        let cleanup = SessionStartCleanup::new(turn_state.clone());
        let input = SessionStartInput {
            common: CommonInput {
                session_id: "session-1".to_string(),
                transcript_path: "/tmp/transcript.jsonl".to_string(),
                cwd: "/tmp".to_string(),
                permission_mode: "default".to_string(),
                hook_event_name: crate::types::HookType::SessionStart,
            },
            source: None,
            model: None,
        };
        let mut ctx = ChainContext::new();
        cleanup.process(&input, &mut ctx).await;

        // Check state was cleared
        let state = turn_state.load("session-1").unwrap();
        assert!(state.changed.is_empty());
        assert!(state.pending.is_empty());
    }

    #[tokio::test]
    async fn test_new_file_detection() {
        let (turn_state, temp_dir) = create_test_turn_state();

        // File doesn't exist yet
        let test_file = temp_dir.path().join("new_file.txt");

        // Run PreToolUse (file doesn't exist)
        let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
        let pre_input = create_pre_tool_use_input(
            "session-1",
            "Write",
            "tool-1",
            serde_json::json!({
                "file_path": test_file.to_string_lossy()
            }),
        );
        let mut ctx = ChainContext::new();
        pre_tracker.process(&pre_input, &mut ctx).await;

        // Check pre-hash is None
        let state = turn_state.load("session-1").unwrap();
        let hashes = state.pending.get("tool-1").unwrap();
        assert!(hashes.get(&test_file).unwrap().is_none());

        // Create the file (simulating Write tool)
        std::fs::write(&test_file, "new content").unwrap();

        // Run PostToolUse
        let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
        let post_input = create_post_tool_use_input(
            "session-1",
            "Write",
            "tool-1",
            serde_json::json!({
                "file_path": test_file.to_string_lossy()
            }),
        );
        post_tracker.process(&post_input, &mut ctx).await;

        // Check change was detected (None -> Some)
        let state = turn_state.load("session-1").unwrap();
        assert!(state.changed.contains(&test_file));
    }
}
