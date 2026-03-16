//! Integration tests for validator tool calls via MCP.
//!
//! These tests verify that the validator chain correctly handles agent responses
//! that involved MCP tool calls (files read-only + code_context). The fixture
//! simulates an agent that:
//! 1. Called `files` (read file) to inspect source code
//! 2. Called `code_context` (get symbol) to look up function metadata
//! 3. Returned a structured pass/fail JSON judgment
//!
//! The PlaybackAgent replays the recorded interaction including tool_call
//! notifications. This verifies the chain parses the response correctly
//! regardless of whether tools were used during the agent's analysis.

mod test_helpers;

use avp_common::{
    chain::ChainFactory,
    turn::TurnStateManager,
    types::{HookOutput, PostToolUseInput},
    validator::ValidatorLoader,
};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use test_helpers::create_context_with_playback;

/// Build a PostToolUseInput simulating a Write to a file with unsafe code.
fn build_unsafe_code_input(temp: &TempDir, session_id: &str) -> PostToolUseInput {
    serde_json::from_value(serde_json::json!({
        "session_id": session_id,
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": {
            "file_path": "src/main.rs",
            "content": "use std::ptr;\n\nfn process_data(raw: *const u8) -> u8 {\n    // UNSAFE: reading from raw pointer\n    unsafe { ptr::read(raw) }\n}\n"
        },
        "tool_response": {
            "filePath": "src/main.rs",
            "success": true
        },
        "tool_use_id": "toolu_unsafe_001"
    }))
    .unwrap()
}

/// Create a custom "no-unsafe" RuleSet in the temp directory.
///
/// This RuleSet triggers on PostToolUse for Write operations and instructs
/// the agent to read the file and check for unsafe blocks.
fn create_unsafe_check_ruleset(temp: &TempDir) {
    let ruleset_dir = temp
        .path()
        .join(".avp")
        .join("validators")
        .join("no-unsafe-code");
    let rules_dir = ruleset_dir.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();

    // RuleSet manifest
    let manifest = r#"---
name: no-unsafe-code
description: Check written files for unsafe Rust code blocks
version: 1.0.0
trigger: PostToolUse
severity: error
match:
  tools:
    - Write
---

# No Unsafe Code

Check written Rust files for unsafe code blocks.
"#;
    fs::write(ruleset_dir.join("VALIDATOR.md"), manifest).unwrap();

    // Rule file
    let rule = r#"---
name: check-unsafe-blocks
description: Read the file and check for unsafe blocks
---

Read the file that was just written using the `files` tool (op: "read file").
Then check if it contains any `unsafe` blocks.

If you find unsafe code, fail with a message describing where the unsafe block is.
If the code is safe, pass.
"#;
    fs::write(rules_dir.join("check-unsafe-blocks.md"), rule).unwrap();
}

/// Execute the PostToolUse chain with a validator fixture that uses MCP tools.
///
/// The fixture records an agent that called `files` (read) and `code_context`
/// (get symbol) before returning its judgment.
async fn execute_validator_with_tools(temp: &TempDir) -> (HookOutput, i32) {
    use avp_common::types::HookType;
    use test_helpers::transform_chain_to_claude_output;

    // Clear CLAUDE_ACP so validators actually execute
    let saved_claude_acp = std::env::var("CLAUDE_ACP").ok();
    std::env::remove_var("CLAUDE_ACP");

    // Create a custom RuleSet that requires tool use
    create_unsafe_check_ruleset(temp);

    let context = create_context_with_playback(temp, "validator_with_tool_calls.json");
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    // Use empty loader + load RuleSets from the project dir only (no builtins)
    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(
            &temp.path().join(".avp").join("validators"),
            avp_common::validator::ValidatorSource::Project,
        )
        .unwrap();

    let factory = ChainFactory::new(Arc::new(context), Arc::new(loader), turn_state);
    let mut chain = factory.post_tool_use_chain();

    let input = build_unsafe_code_input(temp, "test-session");
    let (chain_output, _) = chain.execute(&input).await.unwrap();

    // Restore CLAUDE_ACP if it was previously set
    if let Some(val) = saved_claude_acp {
        std::env::set_var("CLAUDE_ACP", val);
    }

    transform_chain_to_claude_output(chain_output, HookType::PostToolUse)
}

// ============================================================================
// Validator with Tool Calls Tests
// ============================================================================

/// Test that a validator using MCP tools produces a blocking decision.
///
/// The fixture simulates an agent that read the file via `files` tool,
/// found unsafe code, and returned a "failed" judgment.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_validator_with_tool_calls_blocks() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, exit_code) = execute_validator_with_tools(&temp).await;

    assert_eq!(
        output.decision,
        Some("block".to_string()),
        "Validator that used tools and found issues should block"
    );
    assert_eq!(exit_code, 0, "PostToolUse blocking should exit 0");
}

/// Test that the validator's tool-informed reason is preserved in the output.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_validator_with_tool_calls_preserves_reason() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_validator_with_tools(&temp).await;

    let reason = output.reason.expect("Should have a reason");
    assert!(
        reason.contains("unsafe"),
        "Reason should mention the unsafe code found via tool call. Got: {}",
        reason
    );
}

/// Test that the validator result includes continue=true for PostToolUse.
#[tokio::test]
#[serial_test::serial(cwd)]
async fn test_validator_with_tool_calls_continues() {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let (output, _) = execute_validator_with_tools(&temp).await;

    assert!(
        output.continue_execution,
        "PostToolUse blocking must have continue=true (tool already ran)"
    );
}
