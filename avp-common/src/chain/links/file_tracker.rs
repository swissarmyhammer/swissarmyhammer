//! Chain links for tracking file changes during a turn.
//!
//! These links work together to detect which files actually changed:
//! - `PreToolUseFileTracker`: Extracts paths from tool input and hashes files before execution
//! - `PostToolUseFileTracker`: Hashes files after execution and records changes
//! - `SessionStartCleanup`: Clears turn state at session start
//!
//! ## Diagnostic logging — kanban task `01KQ8CXYMBGN1VTV4S89FGQYCA`
//!
//! When a Stop hook fires with `changed_files_count=0`, the proximate cause
//! is always one of:
//!
//! - **Path A** — the writer dropped the change. Three sub-modes: no extracted
//!   paths, no `tool_use_id`, or PostToolUse finds no pending hashes for the
//!   tool_use_id (Pre/Post mismatch). Each sub-mode emits an info-level log
//!   line below so the silent failure is greppable in `.avp/log` without
//!   enabling debug logs.
//! - **Path C** — the writer ran cleanly but the file's pre-hash equalled its
//!   post-hash (a no-op write — same bytes overwriting same bytes). That is
//!   the *correct* outcome: a Write that doesn't actually change file bytes
//!   should not appear in `state.changed`. The "pending hashes present but no
//!   file content actually changed" log line distinguishes Path C from
//!   Path A's no-pending-hashes mode in production logs.
//!
//! The 22:38 production session that motivated this task was Path C: the
//! qwen test re-wrote `sample_avp_test.rs` with the exact same bytes (the
//! transcript's `tool_result.content` and `tool_result.originalFile` are
//! byte-identical), so `state.changed` correctly stayed empty. See the
//! kanban description and the `noop_write_does_not_accumulate_into_changed`
//! test in `avp-common/tests/stop_hook_code_quality_regression.rs` for the
//! locked-in contract.

use std::sync::Arc;

use async_trait::async_trait;

use crate::chain::{ChainContext, ChainLink, ChainResult, CTX_FILE_DIFFS};
use crate::turn::{
    compute_diff, extract_tool_paths, hash_bytes, is_known_file_tool, FileDiff, TurnStateManager,
};
use crate::types::{PostToolUseInput, PreToolUseInput, SessionStartInput};

/// Return the top-level keys of a JSON tool_input value (object only).
///
/// Used by the `tool_input produced 0 paths` diagnostic so we can surface
/// the *shape* of the tool input without leaking its values. For `Write`,
/// `tool_input.content` can carry the full file body, which may include
/// secrets (env files, credentials YAML). Keys are safe to log; values are
/// not.
///
/// Returns an empty vec for non-object JSON values.
fn tool_input_top_level_keys(tool_input: &serde_json::Value) -> Vec<&str> {
    match tool_input {
        serde_json::Value::Object(map) => map.keys().map(|s| s.as_str()).collect(),
        _ => Vec::new(),
    }
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
        // Extract paths from tool input using tool-aware strategy
        let paths = extract_tool_paths(&input.tool_name, &input.tool_input);

        if paths.is_empty() {
            // Known file-modifying tools that produce no extracted paths are a
            // silent failure mode — the Stop hook will see no changed files
            // even though the tool actually wrote one. Surface at info so the
            // condition is greppable without enabling debug logs.
            //
            // We deliberately log only the top-level keys of `tool_input` (not
            // the values), because for `Write` the `content` value can carry
            // secrets — env files, credentials YAML, configs with API keys.
            // Logging the keys is enough to diagnose "the tool fired with
            // unexpected input shape"; logging the values would leak.
            // See kanban task 01KQ8CXYMBGN1VTV4S89FGQYCA.
            if is_known_file_tool(&input.tool_name) {
                let tool_input_keys = tool_input_top_level_keys(&input.tool_input);
                tracing::info!(
                    tool_name = %input.tool_name,
                    tool_input_keys = ?tool_input_keys,
                    "PreToolUseFileTracker: known file tool produced 0 paths — file change will not be tracked",
                );
            } else {
                tracing::trace!(
                    "PreToolUseFileTracker: No paths found in {} tool input",
                    input.tool_name
                );
            }
            return ChainResult::continue_empty();
        }

        // Need a tool_use_id to track this
        let Some(tool_use_id) = &input.tool_use_id else {
            // Same silent-failure surface as the empty-paths branch above:
            // a known file tool firing with no tool_use_id means the change
            // cannot be paired up at PostToolUse time. Surface at info.
            if is_known_file_tool(&input.tool_name) {
                tracing::info!(
                    tool_name = %input.tool_name,
                    "PreToolUseFileTracker: known file tool fired without tool_use_id — file change will not be tracked",
                );
            } else {
                tracing::trace!(
                    "PreToolUseFileTracker: No tool_use_id for {} tool, skipping",
                    input.tool_name
                );
            }
            return ChainResult::continue_empty();
        };

        tracing::debug!(
            "PreToolUseFileTracker: Hashing {} paths for tool {} ({})",
            paths.len(),
            input.tool_name,
            tool_use_id
        );

        let session_id = input.common.session_id.as_deref().unwrap_or_default();

        // Read each file once, hash the bytes, and persist the content to disk.
        // This avoids reading each file twice (once for hashing, once for stashing).
        // Content is written to sidecar files so it survives across process boundaries.
        let mut hashes = std::collections::HashMap::new();
        for path in &paths {
            let content = std::fs::read(path).ok();
            let hash = content.as_deref().map(hash_bytes);
            hashes.insert(path.clone(), hash);
            if let Err(e) =
                self.turn_state
                    .write_pre_content(session_id, tool_use_id, path, content.as_deref())
            {
                tracing::warn!(
                    "PreToolUseFileTracker: Failed to write pre-content for {}: {}",
                    path.display(),
                    e
                );
            }
        }

        // Store hashes in turn state on disk
        match self.turn_state.load(session_id) {
            Ok(mut state) => {
                state.pending.insert(tool_use_id.clone(), hashes);
                if let Err(e) = self.turn_state.save(session_id, &state) {
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
    async fn process(&self, input: &PostToolUseInput, ctx: &mut ChainContext) -> ChainResult {
        let Some(tool_use_id) = &input.tool_use_id else {
            // Known file tool firing without tool_use_id is a silent failure
            // mode — the post-side cannot pair with a Pre snapshot, so the
            // file change never reaches Stop. See kanban task
            // 01KQ8CXYMBGN1VTV4S89FGQYCA.
            if is_known_file_tool(&input.tool_name) {
                tracing::info!(
                    tool_name = %input.tool_name,
                    "PostToolUseFileTracker: known file tool fired without tool_use_id — change not tracked",
                );
            }
            return ChainResult::continue_empty();
        };

        let session_id = input.common.session_id.as_deref().unwrap_or_default();

        // Take pre-content from disk sidecar files (persisted across process boundaries)
        let pre_contents = self.turn_state.take_pre_content(session_id, tool_use_id);
        let mut state = match self.turn_state.load(session_id) {
            Ok(state) => state,
            Err(e) => {
                tracing::warn!("PostToolUseFileTracker: Failed to load turn state: {}", e);
                return ChainResult::continue_empty();
            }
        };

        // Get pending hashes for this tool
        let Some(pre_hashes) = state.pending.remove(tool_use_id) else {
            // Most common silent-failure mode: PostToolUse fires for a known
            // file-modifying tool but no Pre snapshot was recorded under the
            // matching `(session_id, tool_use_id)` pair. This is the kanban
            // task 01KQ8CXYMBGN1VTV4S89FGQYCA regression — the Stop hook
            // later sees `state.changed.is_empty()` and rejects every
            // ruleset with a `match.files` pattern. Surface at info so the
            // absence is greppable in `.avp/log` without enabling debug
            // logs.
            //
            // Two underlying causes both produce this same symptom:
            //   - Pre never ran for this tool (the chain was short-circuited
            //     for the Pre hook but not for the Post — e.g. a subagent
            //     boundary or an env-var change between invocations).
            //   - Pre wrote its snapshot under a different `session_id`
            //     than Post is reading from (the path-B failure mode the
            //     kanban description called out). The `pending_keys` field
            //     covers the second case: if the keys are non-empty, the
            //     mismatch is on `tool_use_id`; if `pending_keys` is empty,
            //     either Pre never ran for this session or its writes were
            //     cleared between Pre and Post.
            if is_known_file_tool(&input.tool_name) {
                tracing::info!(
                    tool_name = %input.tool_name,
                    tool_use_id = %tool_use_id,
                    session_id = %session_id,
                    pending_keys = ?state.pending.keys().collect::<Vec<_>>(),
                    "PostToolUseFileTracker: no Pre snapshot found for this (session_id, tool_use_id) pair — \
                     either Pre was skipped, the session_id changed between Pre and Post, \
                     or the Pre snapshot was cleared before Post ran. Change not tracked.",
                );
            } else {
                tracing::trace!(
                    "PostToolUseFileTracker: No pending hashes for tool {}",
                    tool_use_id
                );
            }
            return ChainResult::continue_empty();
        };

        // Compare hashes and compute diffs
        let mut changed_count = 0;
        let mut diffs: Vec<FileDiff> = Vec::new();

        for (path, pre_hash) in &pre_hashes {
            let post_hash = crate::turn::hash_file(path);

            if *pre_hash != post_hash {
                tracing::debug!(
                    "PostToolUseFileTracker: File changed: {} (pre: {:?}, post: {:?})",
                    path.display(),
                    pre_hash,
                    post_hash
                );

                if !state.changed.contains(path) {
                    state.changed.push(path.clone());
                    changed_count += 1;
                }

                // Compute diff if we have pre-content and the file exists post-tool
                if let Ok(new_content) = std::fs::read(path) {
                    let old_content = pre_contents
                        .as_ref()
                        .and_then(|m| m.get(path))
                        .and_then(|c| c.as_ref());

                    let diff = compute_diff(path, old_content.map(|v| v.as_slice()), &new_content);
                    diffs.push(diff);
                }
            }
        }

        if changed_count > 0 {
            // Surface the per-tool changed-file count at info level so the
            // production .avp/log shows the writer-side evidence that pairs
            // with the Stop hook's `Stop hook resolved changed files
            // changed_files_count=N` info-level line. See kanban task
            // 01KQ8CXYMBGN1VTV4S89FGQYCA — without this log line, an empty
            // Stop changed-files list cannot be diagnosed without enabling
            // debug logs.
            tracing::info!(
                changed_count,
                tool_name = %input.tool_name,
                tool_use_id = %tool_use_id,
                session_id = %session_id,
                "PostToolUseFileTracker: tracked changed files",
            );
        } else if is_known_file_tool(&input.tool_name) {
            // Known file tool fired with pending hashes but the post-tool
            // hash matched the pre-tool hash for every tracked path —
            // i.e. the tool ran but didn't actually change anything. This
            // is uncommon for Write/Edit but surface it so the silent
            // "no change" case is distinguishable in production logs from
            // the silent "no pending hashes" case above.
            tracing::info!(
                tool_name = %input.tool_name,
                tool_use_id = %tool_use_id,
                session_id = %session_id,
                pre_hashes_count = pre_hashes.len(),
                "PostToolUseFileTracker: pending hashes present but no file content actually changed",
            );
        }

        // Write diffs to sidecar files (persisted for Stop validators)
        for diff in &diffs {
            if !diff.diff_text.is_empty() {
                if let Err(e) = self
                    .turn_state
                    .write_diff(session_id, &diff.path, &diff.diff_text)
                {
                    tracing::warn!(
                        "PostToolUseFileTracker: Failed to write sidecar diff for {}: {}",
                        diff.path.display(),
                        e
                    );
                }
            }
        }

        // Put diffs into ChainContext for PostToolUse validators
        if !diffs.is_empty() {
            ctx.set(CTX_FILE_DIFFS, &diffs);
        }

        // Save updated state
        if let Err(e) = self.turn_state.save(session_id, &state) {
            tracing::warn!("PostToolUseFileTracker: Failed to save turn state: {}", e);
        }

        ChainResult::continue_empty()
    }

    fn name(&self) -> &'static str {
        "PostToolUseFileTracker"
    }
}

/// Chain link that clears turn state and diff sidecars on session start.
///
/// This is the natural reset point for a fresh turn. Diffs from the previous
/// turn survive Stop (for post-mortem debugging) and are cleaned here.
pub struct SessionStartCleanup {
    turn_state: Arc<TurnStateManager>,
}

impl SessionStartCleanup {
    /// Create a new session start cleanup link.
    pub fn new(turn_state: Arc<TurnStateManager>) -> Self {
        Self { turn_state }
    }
}

#[async_trait(?Send)]
impl ChainLink<SessionStartInput> for SessionStartCleanup {
    async fn process(&self, input: &SessionStartInput, _ctx: &mut ChainContext) -> ChainResult {
        let session_id = input.common.session_id.as_deref().unwrap_or_default();
        tracing::debug!(
            "SessionStartCleanup: Clearing turn state for session {}",
            session_id
        );

        if let Err(e) = self.turn_state.clear(session_id) {
            tracing::warn!("SessionStartCleanup: Failed to clear turn state: {}", e);
        }

        // Clear sidecar diff files for this session
        if let Err(e) = self.turn_state.clear_diffs(session_id) {
            tracing::warn!("SessionStartCleanup: Failed to clear diffs: {}", e);
        }

        // Clear sidecar pre-content files for this session
        if let Err(e) = self.turn_state.clear_pre_content(session_id) {
            tracing::warn!("SessionStartCleanup: Failed to clear pre-content: {}", e);
        }

        ChainResult::continue_empty()
    }

    fn name(&self) -> &'static str {
        "SessionStartCleanup"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CommonInput;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// `tool_input_top_level_keys` returns just the keys for an object,
    /// dropping the values. This is the redaction that prevents Write's
    /// `content` field (which can carry secrets) from leaking into
    /// info-level logs. See the Warning 2 fix in kanban task
    /// 01KQ8CXYMBGN1VTV4S89FGQYCA.
    #[test]
    fn test_tool_input_top_level_keys_returns_keys_not_values() {
        let input = serde_json::json!({
            "file_path": "/tmp/secrets.env",
            "content": "API_KEY=sk-real-secret-1234567890abcdef",
        });
        let mut keys = tool_input_top_level_keys(&input);
        keys.sort();
        assert_eq!(keys, vec!["content", "file_path"]);
        // The secret value is NOT in the returned keys.
        assert!(!keys.iter().any(|k| k.contains("sk-real-secret")));
    }

    /// Non-object JSON values (string, number, array, null) yield an empty
    /// keys list rather than a partial leak. The caller logs the empty list
    /// alongside the tool name, which is enough to diagnose "tool input was
    /// not an object" without exposing the contents.
    #[test]
    fn test_tool_input_top_level_keys_non_object_returns_empty() {
        assert!(tool_input_top_level_keys(&serde_json::json!("a string")).is_empty());
        assert!(tool_input_top_level_keys(&serde_json::json!(42)).is_empty());
        assert!(tool_input_top_level_keys(&serde_json::json!(["a", "b"])).is_empty());
        assert!(tool_input_top_level_keys(&serde_json::json!(null)).is_empty());
    }

    /// Empty object yields empty keys list (boundary case).
    #[test]
    fn test_tool_input_top_level_keys_empty_object() {
        assert!(tool_input_top_level_keys(&serde_json::json!({})).is_empty());
    }

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
                session_id: Some(session_id.to_string()),
                transcript_path: Some("/tmp/transcript.jsonl".to_string()),
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
                session_id: Some(session_id.to_string()),
                transcript_path: Some("/tmp/transcript.jsonl".to_string()),
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
                session_id: Some("session-1".to_string()),
                transcript_path: Some("/tmp/transcript.jsonl".to_string()),
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

    #[tokio::test]
    async fn test_post_tool_use_produces_diffs_in_context() {
        let (turn_state, temp_dir) = create_test_turn_state();

        let test_file = temp_dir.path().join("diff_test.txt");
        std::fs::write(&test_file, "line 1\nline 2\nline 3\n").unwrap();

        // PreToolUse
        let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
        let pre_input = create_pre_tool_use_input(
            "session-1",
            "Edit",
            "tool-diff",
            serde_json::json!({ "file_path": test_file.to_string_lossy() }),
        );
        let mut ctx = ChainContext::new();
        pre_tracker.process(&pre_input, &mut ctx).await;

        // Modify file
        std::fs::write(&test_file, "line 1\nline 2 changed\nline 3\n").unwrap();

        // PostToolUse
        let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
        let post_input = create_post_tool_use_input(
            "session-1",
            "Edit",
            "tool-diff",
            serde_json::json!({ "file_path": test_file.to_string_lossy() }),
        );
        post_tracker.process(&post_input, &mut ctx).await;

        // Check diffs in ChainContext
        let diffs: Option<Vec<crate::turn::FileDiff>> = ctx.get(CTX_FILE_DIFFS);
        assert!(diffs.is_some(), "Diffs should be in ChainContext");
        let diffs = diffs.unwrap();
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].path, test_file);
        assert!(!diffs[0].is_new_file);
        assert!(!diffs[0].is_binary);
        assert!(diffs[0].diff_text.contains("-line 2"));
        assert!(diffs[0].diff_text.contains("+line 2 changed"));
    }

    #[tokio::test]
    async fn test_new_file_diff_shows_all_additions() {
        let (turn_state, temp_dir) = create_test_turn_state();

        let test_file = temp_dir.path().join("brand_new.txt");

        // PreToolUse (file doesn't exist)
        let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
        let pre_input = create_pre_tool_use_input(
            "session-1",
            "Write",
            "tool-new",
            serde_json::json!({ "file_path": test_file.to_string_lossy() }),
        );
        let mut ctx = ChainContext::new();
        pre_tracker.process(&pre_input, &mut ctx).await;

        // Create file
        std::fs::write(&test_file, "hello world\n").unwrap();

        // PostToolUse
        let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
        let post_input = create_post_tool_use_input(
            "session-1",
            "Write",
            "tool-new",
            serde_json::json!({ "file_path": test_file.to_string_lossy() }),
        );
        post_tracker.process(&post_input, &mut ctx).await;

        let diffs: Option<Vec<crate::turn::FileDiff>> = ctx.get(CTX_FILE_DIFFS);
        assert!(diffs.is_some());
        let diffs = diffs.unwrap();
        assert_eq!(diffs.len(), 1);
        assert!(diffs[0].is_new_file);
        assert!(diffs[0].diff_text.contains("/dev/null"));
        assert!(diffs[0].diff_text.contains("+hello world"));
    }

    /// End-to-end pipeline test: real file change → file tracker diffs → prepare → render.
    ///
    /// This tests the full data flow that `ValidatorExecutorLink::process` performs:
    /// 1. FileTracker computes diffs and puts them in ChainContext
    /// 2. Input is serialized to JSON (same as `serde_json::to_value(input)`)
    /// 3. `prepare_validator_context` strips bloated fields and embeds diff text
    /// 4. `render_hook_context` produces YAML + diff blocks
    ///
    /// This proves the chain wiring produces the correct validator context format.
    #[tokio::test]
    async fn test_full_pipeline_file_change_to_rendered_validator_context() {
        let (turn_state, temp_dir) = create_test_turn_state();

        let test_file = temp_dir.path().join("pipeline_test.rs");
        std::fs::write(&test_file, "fn main() {\n    println!(\"hello\");\n}\n").unwrap();

        // Step 1: PreToolUse stashes content
        let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
        let pre_input = create_pre_tool_use_input(
            "session-1",
            "Edit",
            "tool-pipeline",
            serde_json::json!({ "file_path": test_file.to_string_lossy() }),
        );
        let mut ctx = ChainContext::new();
        pre_tracker.process(&pre_input, &mut ctx).await;

        // Step 2: File is modified (simulating Edit tool execution)
        std::fs::write(
            &test_file,
            "fn main() {\n    println!(\"hello world\");\n}\n",
        )
        .unwrap();

        // Step 3: PostToolUse computes diffs → ChainContext
        let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
        let post_input = create_post_tool_use_input(
            "session-1",
            "Edit",
            "tool-pipeline",
            serde_json::json!({
                "file_path": test_file.to_string_lossy(),
                "old_string": "    println!(\"hello\");",
                "new_string": "    println!(\"hello world\");",
                "replace_all": false
            }),
        );
        // Add realistic tool_result (this is what Claude Code sends)
        let post_input = PostToolUseInput {
            tool_result: Some(serde_json::json!({
                "filePath": test_file.to_string_lossy(),
                "originalFile": "fn main() {\n    println!(\"hello\");\n}\n",
                "oldString": "    println!(\"hello\");",
                "newString": "    println!(\"hello world\");",
                "structuredPatch": [{"lines": ["-old", "+new"]}],
                "replaceAll": false,
                "userModified": false
            })),
            ..post_input
        };
        post_tracker.process(&post_input, &mut ctx).await;

        // Step 4: Read diffs from ChainContext (same as ValidatorExecutorLink does)
        let diffs: Option<Vec<crate::turn::FileDiff>> = ctx.get(CTX_FILE_DIFFS);
        assert!(diffs.is_some(), "Diffs should be in ChainContext");

        // Step 5: Serialize input to JSON (same as ValidatorExecutorLink does)
        let input_json = serde_json::to_value(&post_input).unwrap();

        // Step 6: Prepare context (same as ValidatorExecutorLink does)
        let prepared = crate::turn::prepare_validator_context(input_json, diffs.as_deref());

        // Step 7: Render (same as extract_hook_context_string does)
        let rendered = crate::turn::render_hook_context(&prepared);

        eprintln!("=== FULL PIPELINE OUTPUT ===\n{}\n=== END ===", rendered);

        // ASSERTIONS: the validator sees YAML + diff, not JSON

        // Must have YAML block
        assert!(
            rendered.contains("```yaml"),
            "Should contain ```yaml fence:\n{}",
            rendered
        );

        // Must have diff block with actual file changes
        assert!(
            rendered.contains("```diff"),
            "Should contain ```diff fence:\n{}",
            rendered
        );
        assert!(
            rendered.contains("-    println!(\"hello\");"),
            "Should contain removed line:\n{}",
            rendered
        );
        assert!(
            rendered.contains("+    println!(\"hello world\");"),
            "Should contain added line:\n{}",
            rendered
        );

        // Must NOT have bloated fields
        assert!(
            !rendered.contains("originalFile"),
            "Should NOT contain originalFile:\n{}",
            rendered
        );
        assert!(
            !rendered.contains("structuredPatch"),
            "Should NOT contain structuredPatch:\n{}",
            rendered
        );

        // Must NOT be JSON format
        assert!(
            !rendered.contains("\"tool_name\""),
            "Should NOT contain JSON-quoted keys:\n{}",
            rendered
        );

        // Must have tool metadata in YAML
        assert!(
            rendered.contains("tool_name: Edit"),
            "Should have tool_name in YAML:\n{}",
            rendered
        );
    }
}
