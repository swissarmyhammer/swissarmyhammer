//! Integration tests for PostToolUse hook output format.
//!
//! These tests verify that PostToolUse hooks return the correct Claude Code
//! format when validators block:
//! 1. `decision: "block"` to flag the tool result
//! 2. `reason` provides feedback to Claude about what went wrong
//! 3. `continue: true` because the tool already ran, we're just flagging it
//! 4. Exit code 0 (required for JSON parsing)
//!
//! See: https://code.claude.com/docs/en/hooks#posttooluse-decision-control

mod test_helpers;

use avp_common::{
    chain::ChainFactory,
    context::AvpContext,
    turn::TurnStateManager,
    types::{HookOutput, PostToolUseInput},
    validator::ValidatorLoader,
};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use test_helpers::create_context_with_playback;

// ============================================================================
// PostToolUse Output Format Tests (Unit Tests)
// ============================================================================

#[test]
fn test_post_tool_use_block_output_has_decision_block() {
    let output = HookOutput::post_tool_use_block("Found hardcoded secrets");

    assert_eq!(
        output.decision,
        Some("block".to_string()),
        "PostToolUse blocking output must have decision='block'"
    );
}

#[test]
fn test_post_tool_use_block_output_has_reason() {
    let reason = "blocked by validator 'no-secrets': Found hardcoded API key";
    let output = HookOutput::post_tool_use_block(reason);

    assert_eq!(
        output.reason,
        Some(reason.to_string()),
        "PostToolUse blocking output must have reason set"
    );
}

#[test]
fn test_post_tool_use_block_output_has_continue_true() {
    let output = HookOutput::post_tool_use_block("Found hardcoded secrets");

    assert!(
        output.continue_execution,
        "PostToolUse blocking must have continue=true (tool already ran)"
    );
}

#[test]
fn test_post_tool_use_block_json_format() {
    let output = HookOutput::post_tool_use_block("Found hardcoded secrets");
    let json = serde_json::to_value(&output).unwrap();

    // Verify JSON matches Claude Code expected format
    assert_eq!(
        json.get("decision").and_then(|v| v.as_str()),
        Some("block"),
        "JSON should have decision: 'block'"
    );
    assert!(
        json.get("reason").is_some(),
        "JSON should have reason field"
    );
    assert_eq!(
        json.get("continue").and_then(|v| v.as_bool()),
        Some(true),
        "JSON should have continue: true"
    );
}

// ============================================================================
// PostToolUse Chain Execution Tests
// ============================================================================

/// Build a PostToolUseInput for testing.
fn build_post_tool_use_input(temp: &TempDir, session_id: &str) -> PostToolUseInput {
    serde_json::from_value(serde_json::json!({
        "session_id": session_id,
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": "config.ts",
            "content": "const apiKey = 'sk-proj-1234567890';"
        },
        "tool_response": {
            "filePath": "config.ts",
            "success": true
        },
        "tool_use_id": "toolu_test123"
    }))
    .unwrap()
}

/// Helper: Execute PostToolUse chain with a failing validator and return Claude-specific output.
async fn execute_blocking_post_tool_use_chain(temp: &TempDir) -> (HookOutput, i32) {
    use avp_common::types::HookType;
    use test_helpers::transform_chain_to_claude_output;

    let context = create_context_with_playback(temp, "post_tool_use_no_secrets_fail.json");
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let factory = ChainFactory::new(Arc::new(context), Arc::new(loader), turn_state);
    let mut chain = factory.post_tool_use_chain();

    let input = build_post_tool_use_input(temp, "test-session");

    let (chain_output, _) = chain.execute(&input).await.unwrap();
    // Transform chain output to Claude-specific format for testing
    transform_chain_to_claude_output(chain_output, HookType::PostToolUse)
}

/// Test that PostToolUse hook blocking sets decision="block".
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_post_tool_use_hook_blocking_decision_set() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_post_tool_use_chain(&temp).await;

    assert_eq!(
        output.decision,
        Some("block".to_string()),
        "PostToolUse blocking output must have decision='block'"
    );
}

/// Test that PostToolUse hook blocking provides a reason.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_post_tool_use_hook_blocking_reason_provided() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_post_tool_use_chain(&temp).await;

    assert!(
        output.reason.is_some(),
        "PostToolUse blocking must have a reason"
    );
    assert!(
        output.reason.as_ref().unwrap().contains("validator"),
        "Reason should mention which validator blocked: {:?}",
        output.reason
    );
}

/// Test that PostToolUse hook blocking has continue=true.
///
/// Per Claude Code docs, PostToolUse blocking uses continue=true because
/// the tool has already executed - we're just flagging the result.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_post_tool_use_hook_blocking_has_continue_true() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_post_tool_use_chain(&temp).await;

    assert!(
        output.continue_execution,
        "PostToolUse blocking must have continue=true (tool already ran)"
    );
}

/// Test that PostToolUse hook blocking returns exit code 0.
///
/// Per Claude Code docs, exit code 0 is required for JSON parsing.
/// The decision: "block" + reason in the JSON provides feedback to Claude.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_post_tool_use_hook_blocking_exit_code_0() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (_, exit_code) = execute_blocking_post_tool_use_chain(&temp).await;

    assert_eq!(
        exit_code, 0,
        "PostToolUse blocking should return exit code 0 (JSON format)"
    );
}

/// Test that PostToolUse hook blocking output serializes correctly to JSON.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_post_tool_use_hook_blocking_json_format() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_post_tool_use_chain(&temp).await;
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

// ============================================================================
// Comparison with Stop Hook Behavior
// ============================================================================

/// Test that PostToolUse and Stop hooks both use decision="block" but differ
/// in their semantics.
///
/// - Stop hook blocking: Prevents Claude from stopping (forces continuation)
/// - PostToolUse blocking: Flags the tool result (tool already ran)
///
/// Both should have:
/// - decision: "block"
/// - reason: set
/// - continue: true
/// - exit_code: 0 (JSON format)
#[test]
fn test_post_tool_use_and_stop_block_format_consistency() {
    let post_tool_use_output =
        HookOutput::post_tool_use_block("PostToolUse: Found secrets in written file");
    let stop_output = HookOutput::stop_block("Stop: Must fix issues before stopping");

    // Both should have decision="block"
    assert_eq!(
        post_tool_use_output.decision, stop_output.decision,
        "Both hooks should use decision='block'"
    );

    // Both should have continue=true
    assert_eq!(
        post_tool_use_output.continue_execution, stop_output.continue_execution,
        "Both hooks should have continue=true"
    );

    // Both should have reason set
    assert!(
        post_tool_use_output.reason.is_some() && stop_output.reason.is_some(),
        "Both hooks should have reason set"
    );
}

// ============================================================================
// Chain Execution Without Blocking (Success Path)
// ============================================================================

/// Test that PostToolUse chain succeeds when no validators match/block.
///
/// This test uses an empty validator loader to verify the chain's success path
/// without the complexity of matching multiple validators against fixtures.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_post_tool_use_chain_success_path() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    std::env::set_var("AVP_SKIP_AGENT", "1");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let context = Arc::new(AvpContext::init().unwrap());

    std::env::set_current_dir(original_dir).unwrap();

    // Use empty loader - no validators to execute
    let loader = Arc::new(ValidatorLoader::new());
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    let factory = ChainFactory::new(context, loader, turn_state);
    let mut chain = factory.post_tool_use_chain();

    let input: PostToolUseInput = serde_json::from_value(serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": "config.ts",
            "content": "const apiKey = process.env.API_KEY;"
        },
        "tool_response": {
            "filePath": "config.ts",
            "success": true
        },
        "tool_use_id": "toolu_test456"
    }))
    .unwrap();

    let (chain_output, exit_code) = chain.execute(&input).await.unwrap();

    // Test agent-agnostic chain output
    assert!(
        chain_output.continue_execution,
        "Chain should allow continuation when no validators block"
    );
    assert_eq!(exit_code, 0, "Exit code should be 0 for success");
    assert!(
        chain_output.validator_block.is_none(),
        "Should not have validator block when no validators block"
    );

    std::env::remove_var("AVP_SKIP_AGENT");
}

// ============================================================================
// Skipped Agent Tests (Structure Only)
// ============================================================================

/// Test that PostToolUse chain structure is correct with skipped agent.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_post_tool_use_chain_structure_with_skipped_agent() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    std::env::set_var("AVP_SKIP_AGENT", "1");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let context = Arc::new(AvpContext::init().unwrap());
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    std::env::set_current_dir(original_dir).unwrap();

    let factory = ChainFactory::new(context, Arc::new(loader), turn_state);
    let mut chain = factory.post_tool_use_chain();

    let input = build_post_tool_use_input(&temp, "test-session");
    let (output, exit_code) = chain.execute(&input).await.unwrap();

    // With AVP_SKIP_AGENT, validators are skipped so chain should succeed
    assert!(output.continue_execution, "Chain should allow continuation");
    assert_eq!(exit_code, 0, "Exit code should be 0");

    std::env::remove_var("AVP_SKIP_AGENT");
}
