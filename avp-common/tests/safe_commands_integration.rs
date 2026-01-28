//! Integration tests for the safe-commands validator.
//!
//! These tests verify that the safe-commands validator correctly:
//! 1. Loads from builtins
//! 2. Matches PreToolUse hooks for Bash operations
//! 3. Blocks sed and awk commands
//! 4. Executes via PlaybackAgent for deterministic testing

mod test_helpers;

use avp_common::{
    strategy::ClaudeCodeHookStrategy,
    types::HookType,
    validator::{ValidatorLoader, ValidatorRunner},
};
use test_helpers::{create_context_with_playback, create_test_context, HookInputBuilder};

// ============================================================================
// Validator Loading Tests
// ============================================================================

#[test]
fn test_safe_commands_validator_loads() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let validator = loader.get("safe-commands");
    assert!(
        validator.is_some(),
        "safe-commands validator should be loaded"
    );

    let validator = validator.unwrap();
    assert_eq!(validator.name(), "safe-commands");
    assert!(
        validator.body.contains("sed") && validator.body.contains("awk"),
        "validator body should mention sed and awk"
    );
}

// ============================================================================
// Validator Matching Tests
// ============================================================================

#[test]
#[serial_test::serial(cwd)]
fn test_safe_commands_validator_matches_bash() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    let input = HookInputBuilder::pre_tool_use_bash("sed -i 's/foo/bar/g' file.txt");
    let matching = strategy.matching_validators(HookType::PreToolUse, &input);

    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
    assert!(
        names.contains(&"safe-commands"),
        "safe-commands validator should match PreToolUse + Bash, got: {:?}",
        names
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_safe_commands_validator_does_not_match_write() {
    let (_temp, context) = create_test_context();

    std::env::set_var("AVP_SKIP_AGENT", "1");
    let strategy = ClaudeCodeHookStrategy::new(context);
    std::env::remove_var("AVP_SKIP_AGENT");

    // Write tool should not match safe-commands (which is PreToolUse + Bash)
    let input = serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/test-transcript.jsonl",
        "cwd": "/tmp",
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": "test.txt",
            "content": "hello"
        }
    });

    let matching = strategy.matching_validators(HookType::PreToolUse, &input);
    let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
    assert!(
        !names.contains(&"safe-commands"),
        "safe-commands validator should not match Write tool, but got: {:?}",
        names
    );
}

// ============================================================================
// PlaybackAgent Integration Tests for sed/awk blocking
// ============================================================================

/// Integration test using PlaybackAgent to verify validator blocks sed.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_safe_commands_validator_blocks_sed_playback() {
    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent using the sed-blocked fixture
    let context = create_context_with_playback(&temp, "safe_commands_block_sed.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the safe-commands validator
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("safe-commands").unwrap();

    // Build input with sed command
    let input = HookInputBuilder::pre_tool_use_bash("sed -i 's/foo/bar/g' file.txt");

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PreToolUse, &input, None)
        .await;

    // The validator should FAIL (sed blocked)
    assert!(
        !result.result.passed(),
        "Validator should fail when sed command is used. Got result: {:?}",
        result
    );
    assert!(
        result.result.message().contains("sed"),
        "Message should mention sed: {}",
        result.result.message()
    );
}

/// Integration test using PlaybackAgent to verify validator blocks awk.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_safe_commands_validator_blocks_awk_playback() {
    let (temp, _) = create_test_context();

    // Create context with PlaybackAgent using the awk-blocked fixture
    let context = create_context_with_playback(&temp, "safe_commands_block_awk.json");

    // Get agent from context and create runner
    let (agent, notifications) = context.agent().await.expect("Should get agent");
    let runner = ValidatorRunner::new(agent, notifications).expect("Should create runner");

    // Load the safe-commands validator
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let validator = loader.get("safe-commands").unwrap();

    // Build input with awk command
    let input = HookInputBuilder::pre_tool_use_bash("awk '{print $1}' file.txt");

    // Execute the validator
    let (result, _rate_limited) = runner
        .execute_validator(validator, HookType::PreToolUse, &input, None)
        .await;

    // The validator should FAIL (awk blocked)
    assert!(
        !result.result.passed(),
        "Validator should fail when awk command is used. Got result: {:?}",
        result
    );
    assert!(
        result.result.message().contains("awk"),
        "Message should mention awk: {}",
        result.result.message()
    );
}
