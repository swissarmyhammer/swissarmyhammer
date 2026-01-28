//! Shared test helpers for integration tests.
//!
//! This module provides common utilities used across multiple integration test files
//! to reduce code duplication.

#![allow(dead_code)] // Methods are available for future tests

use agent_client_protocol_extras::PlaybackAgent;
use avp_common::context::AvpContext;
use avp_common::types::{CommonInput, HookType, PostToolUseInput, PreToolUseInput};
use avp_common::validator::ExecutedValidator;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

/// Create a test context in a temporary git repository.
///
/// This sets up a temporary directory with a `.git` folder to simulate
/// a git repository environment, then creates an AvpContext within it.
///
/// Returns the TempDir (which must be kept alive to preserve the directory)
/// and the initialized AvpContext.
pub fn create_test_context() -> (TempDir, AvpContext) {
    let temp = TempDir::new().unwrap();
    fs::create_dir_all(temp.path().join(".git")).unwrap();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let context = AvpContext::init().unwrap();

    std::env::set_current_dir(&original_dir).unwrap();

    (temp, context)
}

/// Get the path to test fixtures directory.
///
/// Returns the path to `.fixtures/claude` relative to the crate's manifest directory.
pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".fixtures/claude")
}

/// Create an AvpContext with a PlaybackAgent for deterministic testing.
///
/// This creates a context that uses a pre-recorded fixture file for agent
/// responses, enabling reproducible tests without actual LLM calls.
///
/// # Arguments
/// * `temp` - The temporary directory to use as the working directory
/// * `fixture_name` - The name of the fixture file (e.g., "validator_pass.json")
pub fn create_context_with_playback(temp: &TempDir, fixture_name: &str) -> AvpContext {
    let fixture_path = fixtures_dir().join(fixture_name);
    let agent = PlaybackAgent::new(fixture_path, "claude");
    let notification_rx = agent.subscribe_notifications();

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let context = AvpContext::with_agent(Arc::new(agent), notification_rx)
        .expect("Should create context with playback agent");

    std::env::set_current_dir(&original_dir).unwrap();
    context
}

/// Builder for hook input JSON values.
///
/// Provides a fluent interface for constructing hook inputs with sensible defaults.
pub struct HookInputBuilder {
    session_id: String,
    transcript_path: String,
    cwd: String,
    permission_mode: String,
    hook_event_name: String,
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
    tool_response: Option<serde_json::Value>,
    tool_use_id: Option<String>,
    stop_hook_active: Option<bool>,
}

impl Default for HookInputBuilder {
    fn default() -> Self {
        Self {
            session_id: "test-session".to_string(),
            transcript_path: "/tmp/test-transcript.jsonl".to_string(),
            cwd: "/tmp".to_string(),
            permission_mode: "default".to_string(),
            hook_event_name: "PostToolUse".to_string(),
            tool_name: None,
            tool_input: None,
            tool_response: None,
            tool_use_id: None,
            stop_hook_active: None,
        }
    }
}

impl HookInputBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the session ID.
    pub fn session_id(mut self, id: &str) -> Self {
        self.session_id = id.to_string();
        self
    }

    /// Set the hook event name.
    pub fn hook_event(mut self, event: &str) -> Self {
        self.hook_event_name = event.to_string();
        self
    }

    /// Set the tool name.
    pub fn tool_name(mut self, name: &str) -> Self {
        self.tool_name = Some(name.to_string());
        self
    }

    /// Set the tool input.
    pub fn tool_input(mut self, input: serde_json::Value) -> Self {
        self.tool_input = Some(input);
        self
    }

    /// Set the tool response.
    pub fn tool_response(mut self, response: serde_json::Value) -> Self {
        self.tool_response = Some(response);
        self
    }

    /// Set the tool use ID.
    pub fn tool_use_id(mut self, id: &str) -> Self {
        self.tool_use_id = Some(id.to_string());
        self
    }

    /// Set the stop hook active flag.
    pub fn stop_hook_active(mut self, active: bool) -> Self {
        self.stop_hook_active = Some(active);
        self
    }

    /// Build a PostToolUse input for a Write operation.
    pub fn post_tool_use_write(file_path: &str, content: &str) -> serde_json::Value {
        Self::new()
            .hook_event("PostToolUse")
            .tool_name("Write")
            .tool_input(serde_json::json!({
                "file_path": file_path,
                "content": content
            }))
            .tool_response(serde_json::json!({
                "filePath": file_path,
                "success": true
            }))
            .tool_use_id("toolu_test123")
            .build()
    }

    /// Build a PreToolUse input for a Bash command.
    pub fn pre_tool_use_bash(command: &str) -> serde_json::Value {
        Self::new()
            .hook_event("PreToolUse")
            .tool_name("Bash")
            .tool_input(serde_json::json!({
                "command": command
            }))
            .build()
    }

    /// Build a Stop hook input.
    pub fn stop(session_id: &str) -> serde_json::Value {
        Self::new()
            .session_id(session_id)
            .hook_event("Stop")
            .stop_hook_active(true)
            .build()
    }

    /// Build the final JSON value.
    pub fn build(self) -> serde_json::Value {
        let mut obj = serde_json::json!({
            "session_id": self.session_id,
            "transcript_path": self.transcript_path,
            "cwd": self.cwd,
            "permission_mode": self.permission_mode,
            "hook_event_name": self.hook_event_name
        });

        if let Some(name) = self.tool_name {
            obj["tool_name"] = serde_json::json!(name);
        }
        if let Some(input) = self.tool_input {
            obj["tool_input"] = input;
        }
        if let Some(response) = self.tool_response {
            obj["tool_response"] = response;
        }
        if let Some(id) = self.tool_use_id {
            obj["tool_use_id"] = serde_json::json!(id);
        }
        if let Some(active) = self.stop_hook_active {
            obj["stop_hook_active"] = serde_json::json!(active);
        }

        obj
    }

    /// Build a CommonInput struct with given session and hook type.
    pub fn common_input(session_id: &str, hook_type: HookType) -> CommonInput {
        CommonInput {
            session_id: session_id.to_string(),
            transcript_path: "/tmp/transcript.jsonl".to_string(),
            cwd: "/tmp".to_string(),
            permission_mode: "default".to_string(),
            hook_event_name: hook_type,
        }
    }

    /// Build a PreToolUseInput for file operations.
    pub fn pre_tool_use_input(
        session_id: &str,
        tool: &str,
        file_path: &str,
        tool_id: &str,
    ) -> PreToolUseInput {
        PreToolUseInput {
            common: Self::common_input(session_id, HookType::PreToolUse),
            tool_name: tool.to_string(),
            tool_input: serde_json::json!({ "file_path": file_path }),
            tool_use_id: Some(tool_id.to_string()),
        }
    }

    /// Build a PostToolUseInput for file operations.
    pub fn post_tool_use_input(
        session_id: &str,
        tool: &str,
        file_path: &str,
        tool_id: &str,
    ) -> PostToolUseInput {
        PostToolUseInput {
            common: Self::common_input(session_id, HookType::PostToolUse),
            tool_name: tool.to_string(),
            tool_input: serde_json::json!({ "file_path": file_path }),
            tool_result: None,
            tool_use_id: Some(tool_id.to_string()),
        }
    }
}

// ============================================================================
// Validator Result Assertion Helpers
// ============================================================================

/// Assert that a validator result passed.
pub fn assert_validator_passed(result: &ExecutedValidator, context: &str) {
    assert!(
        result.result.passed(),
        "Validator should pass {}. Got result: {:?}",
        context,
        result
    );
}

/// Assert that a validator result failed.
pub fn assert_validator_failed(result: &ExecutedValidator, context: &str) {
    assert!(
        !result.result.passed(),
        "Validator should fail {}. Got result: {:?}",
        context,
        result
    );
}

/// Assert that a validator message contains expected text (case-insensitive).
pub fn assert_message_contains(result: &ExecutedValidator, expected: &[&str]) {
    let message = result.result.message().to_lowercase();
    let matched = expected.iter().any(|e| message.contains(&e.to_lowercase()));
    assert!(
        matched,
        "Message should contain one of {:?}, got: {}",
        expected,
        result.result.message()
    );
}

// ============================================================================
// Chain Test Helpers
// ============================================================================

use avp_common::chain::ChainFactory;
use avp_common::turn::TurnStateManager;
use avp_common::validator::ValidatorLoader;

/// Set up a test environment with turn state tracking changed files.
///
/// Returns the turn state manager with the specified files marked as changed.
pub fn setup_turn_state_with_changes(
    temp: &TempDir,
    session_id: &str,
    changed_files: &[&str],
) -> Arc<TurnStateManager> {
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));
    let mut state = avp_common::turn::TurnState::new();
    for file in changed_files {
        state.changed.push(std::path::PathBuf::from(file));
    }
    turn_state.save(session_id, &state).unwrap();
    turn_state
}

/// Create a ChainFactory with skipped agent for testing chain structure.
///
/// Sets AVP_SKIP_AGENT and creates context in the temp directory.
/// Remember to call `cleanup_skip_agent_env()` after the test.
pub fn create_test_chain_factory(
    temp: &TempDir,
    turn_state: Arc<TurnStateManager>,
) -> ChainFactory {
    std::env::set_var("AVP_SKIP_AGENT", "1");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let context = Arc::new(AvpContext::init().unwrap());
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    std::env::set_current_dir(original_dir).unwrap();

    ChainFactory::new(context, Arc::new(loader), turn_state)
}

/// Clean up the AVP_SKIP_AGENT environment variable.
pub fn cleanup_skip_agent_env() {
    std::env::remove_var("AVP_SKIP_AGENT");
}

/// Build a StopInput from a TempDir.
pub fn build_stop_input(temp: &TempDir, session_id: &str) -> avp_common::types::StopInput {
    serde_json::from_value(serde_json::json!({
        "session_id": session_id,
        "transcript_path": "/tmp/transcript.jsonl",
        "cwd": temp.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": true
    }))
    .unwrap()
}

// ============================================================================
// ChainOutput to HookOutput Transformation (for testing Claude-specific output)
// ============================================================================

use avp_common::chain::ChainOutput;
use avp_common::types::{
    HookOutput, HookSpecificOutput, PermissionBehavior, PermissionDecision,
    PermissionRequestDecision, PermissionRequestOutput, PreToolUseOutput,
};

/// Transform agent-agnostic ChainOutput to Claude-specific HookOutput for testing.
///
/// This mirrors the logic in ClaudeCodeHookStrategy::transform_to_claude_output
/// and is used for testing Claude-specific output format in integration tests.
pub fn transform_chain_to_claude_output(
    chain_output: ChainOutput,
    hook_type: HookType,
) -> (HookOutput, i32) {
    // If no validator blocked, return success
    if chain_output.validator_block.is_none() && chain_output.continue_execution {
        return (
            HookOutput {
                continue_execution: true,
                system_message: chain_output.system_message,
                suppress_output: chain_output.suppress_output,
                ..Default::default()
            },
            0,
        );
    }

    // Get validator block info
    let (validator_name, message) = if let Some(ref block) = chain_output.validator_block {
        (block.validator_name.clone(), block.message.clone())
    } else {
        (
            "unknown".to_string(),
            chain_output
                .stop_reason
                .clone()
                .unwrap_or_else(|| "Unknown reason".to_string()),
        )
    };

    let reason = format!("blocked by validator '{}': {}", validator_name, message);

    // Transform based on hook type per Claude Code docs
    match hook_type {
        // PreToolUse: use hookSpecificOutput.permissionDecision: "deny"
        HookType::PreToolUse => {
            let output = HookOutput {
                continue_execution: true,
                hook_specific_output: Some(HookSpecificOutput::PreToolUse(PreToolUseOutput {
                    permission_decision: Some(PermissionDecision::Deny),
                    permission_decision_reason: Some(reason),
                    ..Default::default()
                })),
                system_message: chain_output.system_message,
                suppress_output: chain_output.suppress_output,
                ..Default::default()
            };
            (output, 0)
        }

        // PermissionRequest: use hookSpecificOutput.decision.behavior: "deny"
        HookType::PermissionRequest => {
            let output = HookOutput {
                continue_execution: true,
                hook_specific_output: Some(HookSpecificOutput::PermissionRequest(
                    PermissionRequestOutput {
                        decision: Some(PermissionRequestDecision {
                            behavior: PermissionBehavior::Deny,
                            message: Some(reason),
                            updated_input: None,
                            interrupt: false,
                        }),
                    },
                )),
                system_message: chain_output.system_message,
                suppress_output: chain_output.suppress_output,
                ..Default::default()
            };
            (output, 0)
        }

        // PostToolUse/PostToolUseFailure: decision: "block" + reason
        HookType::PostToolUse | HookType::PostToolUseFailure => {
            let output = HookOutput {
                continue_execution: true,
                decision: Some("block".to_string()),
                reason: Some(reason),
                system_message: chain_output.system_message,
                suppress_output: chain_output.suppress_output,
                ..Default::default()
            };
            (output, 0)
        }

        // Stop/SubagentStop: decision: "block" + reason, continue: true
        HookType::Stop | HookType::SubagentStop => {
            let output = HookOutput {
                continue_execution: true,
                stop_reason: Some(reason.clone()),
                decision: Some("block".to_string()),
                reason: Some(reason),
                system_message: chain_output.system_message,
                suppress_output: chain_output.suppress_output,
                ..Default::default()
            };
            (output, 0)
        }

        // UserPromptSubmit: decision: "block" + reason
        HookType::UserPromptSubmit => {
            let output = HookOutput {
                continue_execution: true,
                decision: Some("block".to_string()),
                reason: Some(reason),
                system_message: chain_output.system_message,
                suppress_output: chain_output.suppress_output,
                ..Default::default()
            };
            (output, 0)
        }

        // Other hooks: use stderr format (exit code 2)
        _ => {
            let output = HookOutput {
                continue_execution: false,
                stop_reason: Some(reason),
                system_message: chain_output.system_message,
                suppress_output: chain_output.suppress_output,
                ..Default::default()
            };
            (output, 2)
        }
    }
}
