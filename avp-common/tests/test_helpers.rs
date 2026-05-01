//! Shared test helpers for integration tests.
//!
//! This module provides common utilities used across multiple integration test files
//! to reduce code duplication.

#![allow(dead_code)] // Methods are available for future tests

use agent_client_protocol_extras::PlaybackAgent;
use std::path::Path;

// ============================================================================
// Validator Fixture Helpers
// ============================================================================

/// Create a minimal valid validator markdown file content.
pub fn minimal_validator(name: &str, description: &str) -> String {
    format!(
        "---\nname: {}\ndescription: {}\ntrigger: PostToolUse\nseverity: warn\n---\n\nValidation instructions for {}.\n",
        name, description, name
    )
}

/// Create a validator with specific settings.
pub fn validator_with_settings(
    name: &str,
    description: &str,
    trigger: &str,
    severity: &str,
) -> String {
    format!(
        "---\nname: {}\ndescription: {}\ntrigger: {}\nseverity: {}\n---\n\nCheck for issues.\n",
        name, description, trigger, severity
    )
}

/// Create a validator directory in a temp path and return the validators subdirectory path.
pub fn create_validator_dir(base: &Path) -> std::path::PathBuf {
    let validators_dir = base.join("validators");
    std::fs::create_dir_all(&validators_dir).unwrap();
    validators_dir
}

// ============================================================================
// RuleSet Fixture Helpers (New Architecture)
// ============================================================================

/// Create a minimal RuleSet manifest (VALIDATOR.md).
pub fn minimal_ruleset_manifest(name: &str, description: &str) -> String {
    format!(
        "---\nname: {}\ndescription: {}\nversion: 1.0.0\ntrigger: PostToolUse\nseverity: warn\n---\n\n# {} RuleSet\n\nRuleSet description.\n",
        name, description, name
    )
}

/// Create a RuleSet manifest with specific settings.
pub fn ruleset_manifest_with_settings(
    name: &str,
    description: &str,
    trigger: &str,
    severity: &str,
) -> String {
    format!(
        "---\nname: {}\ndescription: {}\nversion: 1.0.0\ntrigger: {}\nseverity: {}\n---\n\n# {} RuleSet\n\nRuleSet with custom settings.\n",
        name, description, trigger, severity, name
    )
}

/// Create a minimal rule file.
pub fn minimal_rule(name: &str, description: &str) -> String {
    format!(
        "---\nname: {}\ndescription: {}\n---\n\n# {} Rule\n\nValidation instructions for {}.\n",
        name, description, name, name
    )
}

/// Create a rule with severity override.
pub fn rule_with_severity(name: &str, description: &str, severity: &str) -> String {
    format!(
        "---\nname: {}\ndescription: {}\nseverity: {}\n---\n\n# {} Rule\n\nRule with custom severity.\n",
        name, description, severity, name
    )
}

/// Create a rule with timeout override.
pub fn rule_with_timeout(name: &str, description: &str, timeout: u32) -> String {
    format!(
        "---\nname: {}\ndescription: {}\ntimeout: {}\n---\n\n# {} Rule\n\nRule with custom timeout.\n",
        name, description, timeout, name
    )
}

/// Create a RuleSet directory structure in a base path.
///
/// Creates:
/// - base/validators/ruleset-name/VALIDATOR.md
/// - base/validators/ruleset-name/rules/ directory
///
/// Returns the path to the RuleSet directory.
pub fn create_test_ruleset(base: &Path, ruleset_name: &str) -> std::path::PathBuf {
    let ruleset_dir = base.join("validators").join(ruleset_name);
    std::fs::create_dir_all(ruleset_dir.join("rules")).unwrap();
    ruleset_dir
}
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
/// In ACP 0.11, [`AvpContext::with_agent`] takes any `ConnectTo<Client>`
/// component directly — no `Arc<dyn Agent>` wrapping and no separate
/// notification receiver, since notifications flow through the JSON-RPC
/// connection itself.
///
/// # Arguments
/// * `temp` - The temporary directory to use as the working directory
/// * `fixture_name` - The name of the fixture file (e.g., "validator_pass.json")
pub fn create_context_with_playback(temp: &TempDir, fixture_name: &str) -> AvpContext {
    let fixture_path = fixtures_dir().join(fixture_name);
    let agent = PlaybackAgent::new(fixture_path, "claude");

    let original_dir = std::env::current_dir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();

    let context = AvpContext::with_agent(agent)
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
            session_id: Some(session_id.to_string()),
            transcript_path: Some("/tmp/transcript.jsonl".to_string()),
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

// ============================================================================
// Recording / Playback Integration Helpers
// ============================================================================
//
// Shared helpers for tests that drive the production chain against
// `agent_client_protocol_extras::PlaybackAgent` over a checked-in
// `RecordedSession` fixture. These are extracted from
// `recording_replay_integration.rs` and `validator_block_e2e_integration.rs`
// so the two suites stay in lockstep when the loader format or chain wiring
// changes.

/// Resolve the path to a recording fixture under
/// `tests/fixtures/recordings/`.
///
/// Note this is distinct from [`fixtures_dir`], which points at
/// `.fixtures/claude` (the Claude wire-format fixtures consumed by
/// `create_context_with_playback`). Recording fixtures live alongside the
/// integration tests because they are session-trace JSON, not raw responses.
pub fn recording_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("recordings")
        .join(name)
}

/// Build a [`PlaybackAgent`]-backed [`AvpContext`] in a fresh git worktree.
///
/// Returns the temp dir (which must outlive the context — destroying it
/// removes the `.git` directory) and the configured context.
///
/// `AvpContext::with_agent` looks for the git root from cwd, so this switches
/// to the temp dir while constructing the context, then restores cwd so the
/// helper does not leak directory changes into other tests.
///
/// In ACP 0.11 `PlaybackAgent` is itself a `ConnectTo<Client>` component, so
/// it is passed directly to [`AvpContext::with_agent`]. Notifications flow
/// through the JSON-RPC connection rather than a separate broadcast receiver.
pub fn create_playback_context(fixture: &Path) -> (TempDir, AvpContext) {
    let temp = TempDir::new().expect("tempdir");
    fs::create_dir_all(temp.path().join(".git")).expect("create .git");

    let original_cwd = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(temp.path()).expect("chdir to temp");

    let agent = PlaybackAgent::new(fixture.to_path_buf(), "claude");
    let ctx = AvpContext::with_agent(agent).expect("with_agent");

    std::env::set_current_dir(original_cwd).expect("restore cwd");

    (temp, ctx)
}

/// Drop a single-rule ruleset onto disk under
/// `<temp>/.avp/validators/<ruleset>/` for the given hook trigger and
/// severity. Mirrors the on-disk layout that `ValidatorLoader` expects.
///
/// Both `recording_replay_integration` and `validator_block_e2e_integration`
/// use the same shape — Stop trigger, error severity — but the helper is
/// parameterised so future tests can target other triggers without copying
/// it again.
pub fn write_ruleset_on_disk(
    temp: &TempDir,
    ruleset: &str,
    rule: &str,
    rule_body: &str,
    trigger: &str,
    severity: &str,
) {
    let ruleset_dir = temp.path().join(".avp").join("validators").join(ruleset);
    let rules_dir = ruleset_dir.join("rules");
    fs::create_dir_all(&rules_dir).expect("create rules dir");
    fs::write(
        ruleset_dir.join("VALIDATOR.md"),
        format!(
            "---\nname: {ruleset}\ndescription: Replay-fixture ruleset {ruleset}\nversion: 1.0.0\ntrigger: {trigger}\nseverity: {severity}\n---\n\n# {ruleset} RuleSet\n\nReplay-fixture ruleset.\n",
        ),
    )
    .expect("write VALIDATOR.md");
    fs::write(
        rules_dir.join(format!("{rule}.md")),
        format!("---\nname: {rule}\ndescription: {rule}\n---\n\n# {rule} Rule\n\n{rule_body}\n",),
    )
    .expect("write rule.md");
}

/// Drop a single-rule Stop ruleset onto disk with `severity: error`. Thin
/// wrapper over [`write_ruleset_on_disk`] for the common case both replay
/// integration suites use.
pub fn write_stop_error_ruleset(temp: &TempDir, ruleset: &str, rule: &str, rule_body: &str) {
    write_ruleset_on_disk(temp, ruleset, rule, rule_body, "Stop", "error");
}

/// RAII guard that clears `CLAUDE_ACP` for the lifetime of the test and
/// restores its prior value on drop. Without this, `ValidatorContextStarter`
/// short-circuits the chain when AVP is invoked from inside Claude Code.
///
/// This is RAII-correct: a panic in the test body still runs `Drop`, so the
/// next serial test sees the env var in its original state.
pub struct ClaudeAcpGuard {
    saved: Option<String>,
}

impl ClaudeAcpGuard {
    /// Save the current value of `CLAUDE_ACP` and clear it for the lifetime
    /// of the guard.
    pub fn new() -> Self {
        let saved = std::env::var("CLAUDE_ACP").ok();
        std::env::remove_var("CLAUDE_ACP");
        Self { saved }
    }
}

impl Default for ClaudeAcpGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ClaudeAcpGuard {
    fn drop(&mut self) {
        if let Some(val) = self.saved.take() {
            std::env::set_var("CLAUDE_ACP", val);
        }
    }
}
