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
    strategy::ClaudeCodeHookStrategy,
    turn::TurnStateManager,
    types::HookType,
    validator::{ValidatorLoader, ValidatorRunner},
};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use test_helpers::{
    assert_message_contains, assert_validator_failed, assert_validator_passed,
    build_stop_input, cleanup_skip_agent_env, create_context_with_playback,
    create_test_chain_factory, create_test_context, setup_turn_state_with_changes,
    HookInputBuilder,
};

// ============================================================================
// Validator Loading Tests
// ============================================================================

#[test]
fn test_stop_validators_load() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    // Check that Stop validators are loaded
    let validators = loader.list();
    let stop_validators: Vec<_> = validators
        .iter()
        .filter(|v| v.trigger() == HookType::Stop)
        .collect();

    assert!(
        !stop_validators.is_empty(),
        "Should have at least one Stop validator"
    );

    // code-duplication is the only code-quality validator that remains a Stop validator
    // (others were converted to PostToolUse for per-file checking)
    let validator = loader.get("code-duplication");
    assert!(
        validator.is_some(),
        "Stop validator 'code-duplication' should be loaded"
    );
    assert_eq!(
        validator.unwrap().trigger(),
        HookType::Stop,
        "Validator 'code-duplication' should have Stop trigger"
    );
}

#[test]
fn test_stop_validators_have_no_file_patterns() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let validators = loader.list();
    let stop_validators: Vec<_> = validators
        .iter()
        .filter(|v| v.trigger() == HookType::Stop)
        .collect();

    for validator in stop_validators {
        // Stop validators should not have file patterns
        if let Some(match_criteria) = &validator.frontmatter.match_criteria {
            assert!(
                match_criteria.files.is_empty(),
                "Stop validator '{}' should not have file patterns, but has: {:?}",
                validator.name(),
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
fn test_stop_validators_match_stop_hook() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input = HookInputBuilder::stop("test-session");
    let matching = strategy.matching_validators(HookType::Stop, &input);

    // Should have Stop validators matching
    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();

    // code-duplication is the only Stop validator now
    assert!(
        names.contains(&"code-duplication"),
        "code-duplication should match Stop hook, got: {:?}",
        names
    );

    // code-quality validators are now PostToolUse and should NOT match Stop
    assert!(
        !names.contains(&"cognitive-complexity"),
        "cognitive-complexity should NOT match Stop hook (now PostToolUse)"
    );
    assert!(
        !names.contains(&"no-string-equality"),
        "no-string-equality should NOT match Stop hook (now PostToolUse)"
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_stop_validators_do_not_match_other_hooks() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    // Stop validators should not match PreToolUse
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

    let matching = strategy.matching_validators(HookType::PreToolUse, &pre_input);
    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();

    // Stop validators should NOT match PreToolUse
    assert!(
        !names.contains(&"no-string-equality"),
        "Stop validator should not match PreToolUse"
    );
    assert!(
        !names.contains(&"cognitive-complexity"),
        "Stop validator should not match PreToolUse"
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
    let input =
        HookInputBuilder::pre_tool_use_input("session-1", "Edit", &test_file.to_string_lossy(), "tool-1");
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
    let pre_input =
        HookInputBuilder::pre_tool_use_input("session-1", "Edit", &test_file.to_string_lossy(), "tool-1");
    let mut ctx = ChainContext::new();
    pre_tracker.process(&pre_input, &mut ctx).await;

    // Modify file
    fs::write(&test_file, "modified content").unwrap();

    // Detect change
    let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
    let post_input =
        HookInputBuilder::post_tool_use_input("session-1", "Edit", &test_file.to_string_lossy(), "tool-1");
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
    let pre_input =
        HookInputBuilder::pre_tool_use_input("session-1", "Read", &test_file.to_string_lossy(), "tool-1");
    let mut ctx = ChainContext::new();
    pre_tracker.process(&pre_input, &mut ctx).await;

    // DON'T modify the file - PostToolUse should not detect change
    let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
    let post_input =
        HookInputBuilder::post_tool_use_input("session-1", "Read", &test_file.to_string_lossy(), "tool-1");
    post_tracker.process(&post_input, &mut ctx).await;

    let state = turn_state.load("session-1").unwrap();
    assert!(state.changed.is_empty(), "No change should be recorded");
}

// ============================================================================
// PlaybackAgent Integration Tests
// ============================================================================

/// Integration test: Stop validator passes with non-duplicated code.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_validator_passes_with_changed_files_playback() {
    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent
    let context = create_context_with_playback(&temp, "stop_cognitive_complexity_pass.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the code-duplication validator (only remaining Stop validator)
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("code-duplication").unwrap();

    // Build Stop input
    let input = HookInputBuilder::stop("test-session");

    // Changed files to pass to validator
    let changed_files = vec!["src/lib.rs".to_string(), "src/utils.rs".to_string()];

    // Execute the validator with changed files
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::Stop, &input, Some(&changed_files))
        .await;

    // The validator should PASS (playback fixture returns passing response)
    assert_validator_passed(&result, "for non-duplicated code");
}

/// Integration test: Stop validator fails with duplicated code.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_validator_fails_with_duplicated_code_playback() {
    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent
    let context = create_context_with_playback(&temp, "stop_cognitive_complexity_fail.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the code-duplication validator (only remaining Stop validator)
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("code-duplication").unwrap();

    // Build Stop input
    let input = HookInputBuilder::stop("test-session");

    // Changed files
    let changed_files = vec!["src/duplicated.rs".to_string()];

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::Stop, &input, Some(&changed_files))
        .await;

    // The validator should FAIL (playback fixture returns failing response)
    assert_validator_failed(&result, "for duplicated code");
}

/// Helper: Execute validator with changed files and return result message.
async fn run_validator_with_changed_files(
    temp: &TempDir,
    fixture: &str,
    changed_files: Vec<String>,
) -> (bool, String) {
    let context = create_context_with_playback(temp, fixture);
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    // Use code-duplication validator (the only remaining Stop validator)
    let validator = loader.get("code-duplication").unwrap();

    let input = HookInputBuilder::stop("test-session");
    let (result, _) = runner
        .execute_validator(validator, HookType::Stop, &input, Some(&changed_files))
        .await;

    (result.result.passed(), result.result.message().to_string())
}

/// Test: Validator executes successfully with changed files provided.
///
/// This test verifies that the validator runner correctly accepts and processes
/// a list of changed files. With a playback agent, we can only verify that
/// the execution completes successfully - actual file content validation
/// requires a live agent.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_validator_executes_with_changed_files() {
    let (temp, _) = create_test_context();
    let files = vec![
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
        "src/utils/helpers.rs".to_string(),
    ];

    let (passed, message) =
        run_validator_with_changed_files(&temp, "stop_with_changed_files.json", files).await;

    // Verify the validator executed and returned a valid result
    assert!(passed, "Validator should pass");
    assert!(
        !message.is_empty(),
        "Validator should return a non-empty message"
    );
}

/// Test: Validator response acknowledges changed files.
///
/// This test verifies that the validator processes changed files and returns
/// a meaningful response. The fixture simulates a passing validator that
/// acknowledges it analyzed the provided files.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_stop_validator_response_acknowledges_files() {
    let (temp, _) = create_test_context();
    let files = vec![
        "src/main.rs".to_string(),
        "src/lib.rs".to_string(),
        "src/utils/helpers.rs".to_string(),
    ];

    let (passed, message) =
        run_validator_with_changed_files(&temp, "stop_with_changed_files.json", files).await;

    // Verify the validator passed and returned a response about the files
    assert!(passed, "Validator should pass for files without duplication");
    assert!(
        !message.is_empty(),
        "Response should have a message: {}",
        message
    );
}

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

/// Helper: Execute Stop chain with a failing validator and return Claude-specific output.
async fn execute_blocking_stop_chain(temp: &TempDir) -> (avp_common::types::HookOutput, i32) {
    use avp_common::chain::ChainFactory;
    use avp_common::types::{HookType, StopInput};
    use test_helpers::transform_chain_to_claude_output;

    let context = create_context_with_playback(temp, "stop_cognitive_complexity_fail.json");
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    let mut state = avp_common::turn::TurnState::new();
    state
        .changed
        .push(std::path::PathBuf::from("src/complex.rs"));
    turn_state.save("test-session", &state).unwrap();

    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

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
    // Transform chain output to Claude-specific format for testing
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
fn test_stop_validators_count_matches_validators_not_files() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input = HookInputBuilder::stop("test-session");
    let matching = strategy.matching_validators(HookType::Stop, &input);

    // Count of matching validators should be fixed (based on loaded validators)
    // NOT multiplied by number of changed files
    let validator_count = matching.len();

    // We should have at least 1 Stop validator (code-duplication)
    // Most code-quality validators are now PostToolUse
    assert!(
        validator_count >= 1,
        "Should have at least 1 Stop validator, got: {}",
        validator_count
    );

    // Even with many changed files, the validator count stays the same
    // (This is a design verification - validators run once each with ALL files)
    let input_with_many_files = serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/test-transcript.jsonl",
        "cwd": "/tmp",
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": true,
        // Even if we had file info here, validator count shouldn't change
        "changed_files": ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"]
    });

    let matching_with_files = strategy.matching_validators(HookType::Stop, &input_with_many_files);
    assert_eq!(
        matching.len(),
        matching_with_files.len(),
        "Validator count should not change based on file count"
    );
}
