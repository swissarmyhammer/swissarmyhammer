//! Integration tests for PreToolUse hook output format.
//!
//! These tests verify that PreToolUse hooks return the correct Claude Code
//! format when validators block:
//! 1. `hookSpecificOutput.permissionDecision: "deny"` to block the tool
//! 2. `hookSpecificOutput.permissionDecisionReason` provides feedback to Claude
//! 3. `continue: true` so JSON is parsed (exit code 0)
//!
//! See: https://code.claude.com/docs/en/hooks#pretooluse-decision-control

mod test_helpers;

use avp_common::{
    chain::ChainFactory,
    context::AvpContext,
    turn::TurnStateManager,
    types::{
        HookOutput, HookSpecificOutput, HookType, PermissionDecision, PreToolUseInput,
        PreToolUseOutput,
    },
    validator::ValidatorLoader,
};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use test_helpers::create_context_with_playback;

// ============================================================================
// PreToolUse Output Format Tests (Unit Tests)
// ============================================================================

#[test]
fn test_hook_output_pre_tool_use_deny_has_permission_decision() {
    let output = HookOutput::pre_tool_use_deny("Command not allowed");

    assert!(
        output.hook_specific_output.is_some(),
        "PreToolUse deny must have hookSpecificOutput"
    );

    if let Some(HookSpecificOutput::PreToolUse(pre_tool_use)) = &output.hook_specific_output {
        assert_eq!(
            pre_tool_use.permission_decision,
            Some(PermissionDecision::Deny),
            "PreToolUse blocking must have permissionDecision: deny"
        );
    } else {
        panic!("Expected PreToolUse hook specific output");
    }
}

#[test]
fn test_hook_output_pre_tool_use_deny_has_reason() {
    let reason = "Command 'sed' is not allowed - use the Edit tool instead";
    let output = HookOutput::pre_tool_use_deny(reason);

    if let Some(HookSpecificOutput::PreToolUse(pre_tool_use)) = &output.hook_specific_output {
        assert_eq!(
            pre_tool_use.permission_decision_reason,
            Some(reason.to_string()),
            "PreToolUse blocking must have permissionDecisionReason"
        );
    } else {
        panic!("Expected PreToolUse hook specific output");
    }
}

#[test]
fn test_hook_output_pre_tool_use_deny_has_continue_true() {
    let output = HookOutput::pre_tool_use_deny("Command not allowed");

    assert!(
        output.continue_execution,
        "PreToolUse deny must have continue=true (so JSON is parsed with exit code 0)"
    );
}

#[test]
fn test_hook_output_pre_tool_use_deny_json_format() {
    // Simulate aggregation result
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::PreToolUse(
        PreToolUseOutput {
            permission_decision: Some(PermissionDecision::Deny),
            permission_decision_reason: Some("Command not allowed".to_string()),
            ..Default::default()
        },
    ));

    let json = serde_json::to_value(&output).unwrap();

    // Verify JSON matches Claude Code expected format
    assert_eq!(
        json.get("continue").and_then(|v| v.as_bool()),
        Some(true),
        "JSON should have continue: true"
    );

    let hook_specific = json
        .get("hookSpecificOutput")
        .expect("should have hookSpecificOutput");
    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("PreToolUse"),
        "hookEventName should be PreToolUse"
    );
    assert_eq!(
        hook_specific
            .get("permissionDecision")
            .and_then(|v| v.as_str()),
        Some("deny"),
        "permissionDecision should be deny"
    );
    assert!(
        hook_specific.get("permissionDecisionReason").is_some(),
        "permissionDecisionReason should be present"
    );
}

// ============================================================================
// PreToolUse Chain Execution Tests
// ============================================================================

/// Build a PreToolUseInput for testing with a Bash command.
fn build_pre_tool_use_input(temp: &TempDir, session_id: &str, command: &str) -> PreToolUseInput {
    serde_json::from_value(serde_json::json!({
        "session_id": session_id,
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": command
        },
        "tool_use_id": "toolu_test123"
    }))
    .unwrap()
}

/// Helper: Execute PreToolUse chain with a failing validator and return Claude-specific output.
async fn execute_blocking_pre_tool_use_chain(temp: &TempDir) -> (HookOutput, i32) {
    use test_helpers::transform_chain_to_claude_output;

    let context = create_context_with_playback(temp, "pre_tool_use_safe_commands_fail.json");
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let factory = ChainFactory::new(Arc::new(context), Arc::new(loader), turn_state);
    let mut chain = factory.pre_tool_use_chain();

    // Use sed command which should be blocked by safe-commands validator
    let input = build_pre_tool_use_input(temp, "test-session", "sed -i 's/foo/bar/g' file.txt");

    let (chain_output, _) = chain.execute(&input).await.unwrap();
    // Transform chain output to Claude-specific format for testing
    transform_chain_to_claude_output(chain_output, HookType::PreToolUse)
}

/// Test that PreToolUse hook blocking has hookSpecificOutput with permissionDecision.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_pre_tool_use_hook_blocking_has_permission_decision() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_pre_tool_use_chain(&temp).await;

    assert!(
        output.hook_specific_output.is_some(),
        "PreToolUse blocking must have hookSpecificOutput"
    );

    if let Some(HookSpecificOutput::PreToolUse(pre_tool_use)) = &output.hook_specific_output {
        assert_eq!(
            pre_tool_use.permission_decision,
            Some(PermissionDecision::Deny),
            "PreToolUse blocking must have permissionDecision: deny"
        );
    } else {
        panic!(
            "Expected PreToolUse hook specific output, got: {:?}",
            output.hook_specific_output
        );
    }
}

/// Test that PreToolUse hook blocking provides a permissionDecisionReason.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_pre_tool_use_hook_blocking_has_reason() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_pre_tool_use_chain(&temp).await;

    if let Some(HookSpecificOutput::PreToolUse(pre_tool_use)) = &output.hook_specific_output {
        assert!(
            pre_tool_use.permission_decision_reason.is_some(),
            "PreToolUse blocking must have permissionDecisionReason"
        );
        assert!(
            pre_tool_use
                .permission_decision_reason
                .as_ref()
                .unwrap()
                .contains("validator"),
            "Reason should mention which validator blocked: {:?}",
            pre_tool_use.permission_decision_reason
        );
    } else {
        panic!("Expected PreToolUse hook specific output");
    }
}

/// Test that PreToolUse hook blocking has continue=true.
///
/// Per Claude Code docs, PreToolUse uses hookSpecificOutput for decisions,
/// which requires exit code 0 (JSON parsing). continue=true ensures exit code 0.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_pre_tool_use_hook_blocking_has_continue_true() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_pre_tool_use_chain(&temp).await;

    assert!(
        output.continue_execution,
        "PreToolUse blocking must have continue=true (so JSON is parsed)"
    );
}

/// Test that PreToolUse hook blocking returns exit code 0.
///
/// Per Claude Code docs, exit code 0 is required for JSON parsing.
/// The permissionDecision: "deny" in the JSON handles the actual blocking.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_pre_tool_use_hook_blocking_exit_code_0() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (_, exit_code) = execute_blocking_pre_tool_use_chain(&temp).await;

    assert_eq!(
        exit_code, 0,
        "PreToolUse blocking should return exit code 0 (JSON format)"
    );
}

/// Test that PreToolUse hook blocking output serializes correctly to JSON.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_pre_tool_use_hook_blocking_json_format() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_blocking_pre_tool_use_chain(&temp).await;
    let json = serde_json::to_value(&output).unwrap();

    // Verify JSON structure matches Claude Code expected format
    assert_eq!(
        json.get("continue").and_then(|v| v.as_bool()),
        Some(true),
        "JSON continue should be true"
    );

    let hook_specific = json
        .get("hookSpecificOutput")
        .expect("JSON should have hookSpecificOutput");
    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("PreToolUse"),
        "hookEventName should be PreToolUse"
    );
    assert_eq!(
        hook_specific
            .get("permissionDecision")
            .and_then(|v| v.as_str()),
        Some("deny"),
        "permissionDecision should be deny"
    );
    assert!(
        hook_specific.get("permissionDecisionReason").is_some(),
        "permissionDecisionReason should be present"
    );
}

// ============================================================================
// Chain Execution Without Blocking (Success Path)
// ============================================================================

/// Test that PreToolUse chain succeeds when no validators block.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_pre_tool_use_chain_success_path() {
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
    let mut chain = factory.pre_tool_use_chain();

    // Safe command that wouldn't be blocked
    let input: PreToolUseInput = serde_json::from_value(serde_json::json!({
        "session_id": "test-session",
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {
            "command": "ls -la"
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
        "Should not have validator block when validators pass"
    );

    std::env::remove_var("AVP_SKIP_AGENT");
}

// ============================================================================
// Comparison: Old vs New PreToolUse Format
// ============================================================================

/// Test that PreToolUse transformation uses hookSpecificOutput (not deprecated decision field).
#[test]
fn test_pre_tool_use_uses_hook_specific_output_not_deprecated_decision() {
    use avp_common::chain::{ChainOutput, ValidatorBlockInfo};
    use test_helpers::transform_chain_to_claude_output;

    // Create a chain output with a validator block
    let chain_output = ChainOutput {
        continue_execution: false,
        stop_reason: Some("Command not allowed".to_string()),
        validator_block: Some(ValidatorBlockInfo {
            validator_name: "safe-commands".to_string(),
            message: "Command not allowed".to_string(),
            hook_type: HookType::PreToolUse,
        }),
        ..Default::default()
    };

    // Transform to Claude format
    let (output, _) = transform_chain_to_claude_output(chain_output, HookType::PreToolUse);

    // Should NOT use deprecated decision field at top level
    assert!(
        output.decision.is_none(),
        "PreToolUse should NOT use deprecated top-level decision field"
    );

    // Should use hookSpecificOutput.permissionDecision instead
    assert!(
        output.hook_specific_output.is_some(),
        "PreToolUse should use hookSpecificOutput"
    );

    if let Some(HookSpecificOutput::PreToolUse(pre_tool_use)) = &output.hook_specific_output {
        assert_eq!(
            pre_tool_use.permission_decision,
            Some(PermissionDecision::Deny),
            "Should use hookSpecificOutput.permissionDecision: deny"
        );
    }
}
