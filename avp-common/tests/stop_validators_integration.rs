//! Integration tests for Stop hook validators.
//!
//! These tests verify that:
//! 1. Stop validators are loaded from builtins
//! 2. Stop validators match Stop hook events (no file filtering)
//! 3. Changed files are tracked and passed to validators
//! 4. File change tracking works through the full flow
//! 5. Stop validators execute via PlaybackAgent with changed files in prompt
//! 6. Full chain execution works for Stop hooks

mod test_helpers;

use avp_common::{
    strategy::ClaudeCodeHookStrategy, turn::TurnStateManager, types::HookType,
    validator::ValidatorLoader,
};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use test_helpers::{
    build_stop_input, cleanup_skip_agent_env, create_context_with_playback,
    create_test_chain_factory, create_test_context, setup_turn_state_with_changes,
    HookInputBuilder,
};

// ============================================================================
// Validator Loading Tests
// ============================================================================

#[test]
fn test_stop_rulesets_load() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    // Check that Stop RuleSets are loaded
    let rulesets = loader.list_rulesets();
    let stop_rulesets: Vec<_> = rulesets
        .iter()
        .filter(|rs| rs.trigger() == HookType::Stop)
        .collect();

    // session-lifecycle was the only Stop RuleSet and has been removed
    assert!(
        stop_rulesets.is_empty(),
        "No builtin Stop RuleSets should be loaded (session-lifecycle removed)"
    );
    assert!(
        loader.get_ruleset("session-lifecycle").is_none(),
        "session-lifecycle should not be loaded (removed)"
    );
}

#[test]
fn test_stop_rulesets_have_no_file_patterns() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let rulesets = loader.list_rulesets();
    let stop_rulesets: Vec<_> = rulesets
        .iter()
        .filter(|rs| rs.trigger() == HookType::Stop)
        .collect();

    for ruleset in stop_rulesets {
        // Stop RuleSets should not have file patterns
        if let Some(match_criteria) = &ruleset.manifest.match_criteria {
            assert!(
                match_criteria.files.is_empty(),
                "Stop RuleSet '{}' should not have file patterns, but has: {:?}",
                ruleset.name(),
                match_criteria.files
            );
        }
    }
}

// ============================================================================
// Validator Matching Tests
// ============================================================================

#[test]
#[serial_test::serial(cwd)]
fn test_stop_rulesets_match_stop_hook() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input = HookInputBuilder::stop("test-session");
    let matching = strategy.matching_rulesets(HookType::Stop, &input);

    // Should have Stop RuleSets matching
    let names: Vec<_> = matching.iter().map(|rs| rs.name()).collect();

    // session-lifecycle was removed; verify it does not match
    assert!(
        !names.contains(&"session-lifecycle"),
        "session-lifecycle should not match Stop hook (removed)"
    );

    // code-quality RuleSet is PostToolUse and should NOT match Stop
    assert!(
        !names.contains(&"code-quality"),
        "code-quality should NOT match Stop hook (is PostToolUse)"
    );
    assert!(
        !names.contains(&"security-rules"),
        "security-rules should NOT match Stop hook (is PostToolUse)"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_stop_rulesets_do_not_match_other_hooks() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    // Stop RuleSets should not match PreToolUse
    let pre_input = serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/test-transcript.jsonl",
        "cwd": "/tmp",
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": "test.rs",
            "content": "fn main() {}"
        }
    });

    let matching = strategy.matching_rulesets(HookType::PreToolUse, &pre_input);
    let names: Vec<_> = matching.iter().map(|rs| rs.name()).collect();

    // Stop RuleSets should NOT match PreToolUse
    assert!(
        !names.contains(&"session-lifecycle"),
        "Removed Stop RuleSet should not match PreToolUse"
    );
    assert!(
        !names.contains(&"code-quality"),
        "code-quality RuleSet should not match PreToolUse (only PostToolUse)"
    );
}

// ============================================================================
// File Change Tracking Tests
// ============================================================================

#[test]
fn test_turn_state_manager_tracks_changes() {
    let temp = TempDir::new().unwrap();
    let manager = TurnStateManager::new(temp.path());

    // Load initial state (should be empty)
    let state = manager.load("session-1").unwrap();
    assert!(state.changed.is_empty());
    assert!(state.pending.is_empty());

    // Add a changed file
    let mut state = state;
    state
        .changed
        .push(std::path::PathBuf::from("/test/file.rs"));
    manager.save("session-1", &state).unwrap();

    // Reload and verify
    let loaded = manager.load("session-1").unwrap();
    assert_eq!(loaded.changed.len(), 1);
    assert_eq!(loaded.changed[0], std::path::PathBuf::from("/test/file.rs"));
}

#[test]
fn test_turn_state_cleared_between_sessions() {
    let temp = TempDir::new().unwrap();
    let manager = TurnStateManager::new(temp.path());

    // Add state for session-1
    let mut state = avp_common::turn::TurnState::new();
    state
        .changed
        .push(std::path::PathBuf::from("/test/file.rs"));
    manager.save("session-1", &state).unwrap();

    // Clear the session
    manager.clear("session-1").unwrap();

    // Should be empty now
    let loaded = manager.load("session-1").unwrap();
    assert!(loaded.changed.is_empty());
}

// ============================================================================
// Chain Link Tests
// ============================================================================

use avp_common::chain::links::{
    PostToolUseFileTracker, PreToolUseFileTracker, SessionStartCleanup,
};
use avp_common::chain::{ChainContext, ChainLink};
use avp_common::types::SessionStartInput;

#[tokio::test]
async fn test_file_tracker_records_pending_hash() {
    let temp = TempDir::new().unwrap();
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));
    let test_file = temp.path().join("test.txt");
    fs::write(&test_file, "original content").unwrap();

    let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
    let input = HookInputBuilder::pre_tool_use_input(
        "session-1",
        "Edit",
        &test_file.to_string_lossy(),
        "tool-1",
    );
    let mut ctx = ChainContext::new();
    pre_tracker.process(&input, &mut ctx).await;

    let state = turn_state.load("session-1").unwrap();
    assert!(
        state.pending.contains_key("tool-1"),
        "Should record pending hash"
    );
}

#[tokio::test]
async fn test_file_tracker_detects_change() {
    let temp = TempDir::new().unwrap();
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));
    let test_file = temp.path().join("test.txt");
    fs::write(&test_file, "original content").unwrap();

    // Record pre-hash
    let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
    let pre_input = HookInputBuilder::pre_tool_use_input(
        "session-1",
        "Edit",
        &test_file.to_string_lossy(),
        "tool-1",
    );
    let mut ctx = ChainContext::new();
    pre_tracker.process(&pre_input, &mut ctx).await;

    // Modify file
    fs::write(&test_file, "modified content").unwrap();

    // Detect change
    let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
    let post_input = HookInputBuilder::post_tool_use_input(
        "session-1",
        "Edit",
        &test_file.to_string_lossy(),
        "tool-1",
    );
    post_tracker.process(&post_input, &mut ctx).await;

    let state = turn_state.load("session-1").unwrap();
    assert!(state.changed.contains(&test_file), "Should detect change");
    assert!(state.pending.is_empty(), "Pending should be cleared");
}

#[tokio::test]
async fn test_session_start_cleanup() {
    let temp = TempDir::new().unwrap();
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    let cleanup = SessionStartCleanup::new(turn_state.clone());
    let input = SessionStartInput {
        common: HookInputBuilder::common_input("session-1", HookType::SessionStart),
        source: None,
        model: None,
    };
    let mut ctx = ChainContext::new();
    cleanup.process(&input, &mut ctx).await;

    let state = turn_state.load("session-1").unwrap();
    assert!(state.changed.is_empty(), "State should be cleared");
}

#[tokio::test]
async fn test_file_tracker_no_change_detected_when_unchanged() {
    let temp = TempDir::new().unwrap();
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));
    let test_file = temp.path().join("test.txt");
    fs::write(&test_file, "original content").unwrap();

    // PreToolUse records file hash
    let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
    let pre_input = HookInputBuilder::pre_tool_use_input(
        "session-1",
        "Read",
        &test_file.to_string_lossy(),
        "tool-1",
    );
    let mut ctx = ChainContext::new();
    pre_tracker.process(&pre_input, &mut ctx).await;

    // DON'T modify the file - PostToolUse should not detect change
    let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
    let post_input = HookInputBuilder::post_tool_use_input(
        "session-1",
        "Read",
        &test_file.to_string_lossy(),
        "tool-1",
    );
    post_tracker.process(&post_input, &mut ctx).await;

    let state = turn_state.load("session-1").unwrap();
    assert!(state.changed.is_empty(), "No change should be recorded");
}

// ============================================================================
// PlaybackAgent Integration Tests
// ============================================================================
// NOTE: Direct PlaybackAgent tests for execute_ruleset are removed because
// the session-based execution model (initialize -> new_session -> prompt per rule)
// requires multi-turn PlaybackAgent fixtures that don't exist yet.
// The chain-level tests below still work because AVP_SKIP_AGENT bypasses execution.

/// Integration test: Full chain execution for Stop hook.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_chain_executes_validators() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let turn_state =
        setup_turn_state_with_changes(&temp, "test-session", &["src/lib.rs", "src/main.rs"]);
    let factory = create_test_chain_factory(&temp, turn_state);
    let mut chain = factory.stop_chain();

    let input = build_stop_input(&temp, "test-session");
    let (output, exit_code) = chain.execute(&input).await.unwrap();

    // With AVP_SKIP_AGENT, validators are skipped so chain should succeed
    assert!(output.continue_execution, "Chain should allow continuation");
    assert_eq!(exit_code, 0, "Exit code should be 0");

    cleanup_skip_agent_env();
}

/// Helper: Create a test Stop RuleSet on disk in the temp directory.
///
/// This replaces the removed builtin session-lifecycle for testing the
/// Stop hook blocking mechanism.
fn create_test_stop_ruleset(temp: &TempDir) {
    use test_helpers::{minimal_rule, ruleset_manifest_with_settings};

    let ruleset_dir = temp.path().join("validators").join("test-stop-ruleset");
    fs::create_dir_all(ruleset_dir.join("rules")).unwrap();

    fs::write(
        ruleset_dir.join("VALIDATOR.md"),
        ruleset_manifest_with_settings(
            "test-stop-ruleset",
            "Test Stop RuleSet for integration tests",
            "Stop",
            "error",
        ),
    )
    .unwrap();

    fs::write(
        ruleset_dir.join("rules").join("test-rule.md"),
        minimal_rule("test-rule", "Test rule for Stop hook validation"),
    )
    .unwrap();
}

/// Helper: Execute Stop chain with a test Stop RuleSet and return Claude-specific output.
///
/// Uses a playback fixture that simulates a failing validator to test the
/// blocking output format (decision, reason, JSON serialization).
async fn execute_blocking_stop_chain(temp: &TempDir) -> (avp_common::types::HookOutput, i32) {
    use avp_common::chain::ChainFactory;
    use avp_common::types::{HookType, StopInput};
    use avp_common::validator::ValidatorSource;
    use test_helpers::transform_chain_to_claude_output;

    // Clear CLAUDE_ACP so ValidatorContextStarter doesn't short-circuit.
    // This env var is set when running inside a Claude Code session but
    // tests need validators to actually execute.
    let saved_claude_acp = std::env::var("CLAUDE_ACP").ok();
    std::env::remove_var("CLAUDE_ACP");

    create_test_stop_ruleset(temp);

    let context = create_context_with_playback(temp, "stop_cognitive_complexity_fail.json");
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    let mut state = avp_common::turn::TurnState::new();
    state
        .changed
        .push(std::path::PathBuf::from("src/complex.rs"));
    turn_state.save("test-session", &state).unwrap();

    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    loader
        .load_rulesets_directory(&temp.path().join("validators"), ValidatorSource::Project)
        .unwrap();

    let factory = ChainFactory::new(Arc::new(context), Arc::new(loader), turn_state);
    let mut chain = factory.stop_chain();

    let input: StopInput = serde_json::from_value(serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": true
    }))
    .unwrap();

    let (chain_output, _) = chain.execute(&input).await.unwrap();

    // Restore CLAUDE_ACP if it was previously set
    if let Some(val) = saved_claude_acp {
        std::env::set_var("CLAUDE_ACP", val);
    }

    transform_chain_to_claude_output(chain_output, HookType::Stop)
}

/// Test that Stop hook blocking sets decision="block".
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_hook_blocking_decision_set() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_stop_chain(&temp).await;

    assert_eq!(
        output.decision,
        Some("block".to_string()),
        "Stop blocking output must have decision='block'"
    );
}

/// Test that Stop hook blocking provides a reason.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_hook_blocking_reason_provided() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_stop_chain(&temp).await;

    assert!(output.reason.is_some(), "Stop blocking must have a reason");
    assert!(
        output.reason.as_ref().unwrap().contains("validator"),
        "Reason should mention which validator blocked: {:?}",
        output.reason
    );
}

/// Test that Stop hook blocking allows continuation (Claude cannot stop).
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_hook_blocking_allows_continuation() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, exit_code) = execute_blocking_stop_chain(&temp).await;

    assert!(
        output.continue_execution,
        "Stop blocking must have continue=true (Claude cannot stop)"
    );
    assert_eq!(exit_code, 0, "Exit code should be 0 for JSON format");
}

/// Test that Stop hook blocking output serializes correctly to JSON.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_hook_blocking_json_format() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_stop_chain(&temp).await;
    let json = serde_json::to_value(&output).unwrap();

    assert!(
        json.get("decision").is_some(),
        "JSON should have decision field"
    );
    assert!(
        json.get("reason").is_some(),
        "JSON should have reason field"
    );
    assert_eq!(
        json.get("continue").and_then(|v| v.as_bool()),
        Some(true),
        "JSON continue should be true"
    );
}

/// Test that Stop validators run in parallel, not per-file.
#[test]
#[serial_test::serial(cwd)]
fn test_stop_rulesets_count_matches_rulesets_not_files() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input = HookInputBuilder::stop("test-session");
    let matching = strategy.matching_rulesets(HookType::Stop, &input);

    // Count of matching RuleSets should be fixed (based on loaded RuleSets)
    // NOT multiplied by number of changed files
    let ruleset_count = matching.len();

    // session-lifecycle was the only Stop RuleSet; with it removed, expect 0
    assert_eq!(
        ruleset_count, 0,
        "No builtin Stop RuleSets should be loaded (session-lifecycle removed), got: {}",
        ruleset_count
    );

    // Even with many changed files, the RuleSet count stays the same
    // (This is a design verification - RuleSets run once each with ALL files)
    let input_with_many_files = serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/test-transcript.jsonl",
        "cwd": "/tmp",
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": true,
        // Even if we had file info here, RuleSet count shouldn't change
        "changed_files": ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]
    });

    let matching_with_files = strategy.matching_rulesets(HookType::Stop, &input_with_many_files);
    assert_eq!(
        matching.len(),
        matching_with_files.len(),
        "RuleSet count should not change based on file count"
    );
}
