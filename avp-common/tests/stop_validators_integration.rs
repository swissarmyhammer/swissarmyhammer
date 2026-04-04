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
    validator::{ValidatorLoader, ValidatorRenderContext},
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

    // code-quality and test-integrity are now Stop RuleSets
    assert!(
        stop_rulesets.len() >= 2,
        "Should have at least 2 builtin Stop RuleSets (code-quality, test-integrity), got {}",
        stop_rulesets.len()
    );
    let stop_names: Vec<&str> = stop_rulesets.iter().map(|rs| rs.name()).collect();
    assert!(
        stop_names.contains(&"code-quality"),
        "code-quality should be a Stop RuleSet"
    );
    assert!(
        stop_names.contains(&"test-integrity"),
        "test-integrity should be a Stop RuleSet"
    );
    assert!(
        loader.get_ruleset("session-lifecycle").is_none(),
        "session-lifecycle should not be loaded (removed)"
    );
}

#[test]
fn test_stop_rulesets_retain_file_patterns() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    let rulesets = loader.list_rulesets();
    let stop_rulesets: Vec<_> = rulesets
        .iter()
        .filter(|rs| rs.trigger() == HookType::Stop)
        .collect();

    // code-quality and test-integrity Stop RuleSets should retain file patterns
    // to filter against accumulated changed files
    for ruleset in stop_rulesets {
        if ruleset.name() == "code-quality" || ruleset.name() == "test-integrity" {
            let match_criteria = ruleset.manifest.match_criteria.as_ref().expect(&format!(
                "Stop RuleSet '{}' should have match criteria with file patterns",
                ruleset.name()
            ));
            assert!(
                !match_criteria.files.is_empty(),
                "Stop RuleSet '{}' should retain file patterns for filtering changed files",
                ruleset.name()
            );
            // Should NOT have tool patterns (Stop hooks have no tool_name)
            assert!(
                match_criteria.tools.is_empty(),
                "Stop RuleSet '{}' should not have tool patterns, but has: {:?}",
                ruleset.name(),
                match_criteria.tools
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
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    // Stop RuleSets with file patterns need changed_files to match.
    // Provide source files so code-quality and test-integrity match.
    let ctx = avp_common::validator::MatchContext::new(HookType::Stop).with_changed_files(vec![
        "src/main.rs".to_string(),
        "tests/test_main.rs".to_string(),
    ]);
    let matching = loader.matching_rulesets(&ctx);

    let names: Vec<_> = matching.iter().map(|rs| rs.name()).collect();

    // session-lifecycle was removed; verify it does not match
    assert!(
        !names.contains(&"session-lifecycle"),
        "session-lifecycle should not match Stop hook (removed)"
    );

    // code-quality and test-integrity are now Stop RuleSets and should match
    assert!(
        names.contains(&"code-quality"),
        "code-quality should match Stop hook (migrated to Stop trigger)"
    );
    assert!(
        names.contains(&"test-integrity"),
        "test-integrity should match Stop hook (migrated to Stop trigger)"
    );

    // security-rules remains PostToolUse and should NOT match Stop
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
        "code-quality RuleSet should not match PreToolUse (is Stop trigger)"
    );
    assert!(
        !names.contains(&"test-integrity"),
        "test-integrity RuleSet should not match PreToolUse (is Stop trigger)"
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

// ============================================================================
// Sidecar Diff Lifecycle Tests
// ============================================================================

/// Integration test: sidecar diffs written by PostToolUseFileTracker are readable
/// via TurnStateManager::load_all_diffs (the same path Stop validators use).
///
/// This wires together the write path (PostToolUse) and read path (Stop) to prove
/// they share the same directory structure and encoding.
#[tokio::test]
async fn test_post_tool_use_writes_sidecar_diff_readable_at_stop() {
    let temp = TempDir::new().unwrap();
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));
    let session_id = "sidecar-test-session";

    // 1. Create a file with initial content
    let test_file = temp.path().join("test.txt");
    fs::write(&test_file, "line 1\nline 2\nline 3\n").unwrap();

    // 2. Run PreToolUseFileTracker to stash pre-content and hashes
    let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
    let pre_input = HookInputBuilder::pre_tool_use_input(
        session_id,
        "Edit",
        &test_file.to_string_lossy(),
        "tool-sidecar-1",
    );
    let mut ctx = ChainContext::new();
    pre_tracker.process(&pre_input, &mut ctx).await;

    // 3. Modify the file (simulating the tool executing)
    fs::write(&test_file, "line 1\nline 2 modified\nline 3\n").unwrap();

    // 4. Run PostToolUseFileTracker — this should write a sidecar diff to disk
    let post_tracker = PostToolUseFileTracker::new(turn_state.clone());
    let post_input = HookInputBuilder::post_tool_use_input(
        session_id,
        "Edit",
        &test_file.to_string_lossy(),
        "tool-sidecar-1",
    );
    post_tracker.process(&post_input, &mut ctx).await;

    // 5. Verify the sidecar diff file exists on disk
    let diff_on_disk = turn_state.load_diff(session_id, &test_file);
    assert!(
        diff_on_disk.is_some(),
        "Sidecar diff file should exist on disk after PostToolUse"
    );
    let diff_text = diff_on_disk.unwrap();
    assert!(
        !diff_text.is_empty(),
        "Sidecar diff text should not be empty"
    );

    // 6. Load all diffs the way Stop validators do
    let all_diffs = turn_state.load_all_diffs(session_id);
    assert_eq!(
        all_diffs.len(),
        1,
        "Should have exactly 1 sidecar diff for this session"
    );

    // 7. Assert the loaded diff contains the correct path and diff text
    let file_path_str = test_file.display().to_string();
    let loaded_diff = all_diffs.get(&file_path_str);
    assert!(
        loaded_diff.is_some(),
        "load_all_diffs should contain the changed file path '{}', got keys: {:?}",
        file_path_str,
        all_diffs.keys().collect::<Vec<_>>()
    );

    let loaded_text = loaded_diff.unwrap();
    assert!(
        loaded_text.contains("-line 2"),
        "Diff should contain removed line, got:\n{}",
        loaded_text
    );
    assert!(
        loaded_text.contains("+line 2 modified"),
        "Diff should contain added line, got:\n{}",
        loaded_text
    );
    assert!(
        loaded_text.contains("---"),
        "Diff should have unified diff header"
    );
    assert!(
        loaded_text.contains("+++"),
        "Diff should have unified diff header"
    );
}

/// Test that Stop validators run in parallel, not per-file.
/// The number of matching RuleSets stays constant regardless of how many files changed.
#[test]
fn test_stop_rulesets_count_matches_rulesets_not_files() {
    let mut loader = ValidatorLoader::new();
    avp_common::load_builtins(&mut loader);

    // Match with a few changed files
    let ctx_few = avp_common::validator::MatchContext::new(HookType::Stop)
        .with_changed_files(vec!["src/main.rs".to_string()]);
    let matching_few = loader.matching_rulesets(&ctx_few);

    // code-quality and test-integrity are Stop RuleSets
    assert!(
        matching_few.len() >= 2,
        "Should have at least 2 builtin Stop RuleSets (code-quality, test-integrity), got: {}",
        matching_few.len()
    );

    // Even with many changed files, the RuleSet count stays the same
    // (This is a design verification - RuleSets run once each with ALL files)
    let ctx_many =
        avp_common::validator::MatchContext::new(HookType::Stop).with_changed_files(vec![
            "a.rs".to_string(),
            "b.rs".to_string(),
            "c.rs".to_string(),
            "d.rs".to_string(),
            "e.rs".to_string(),
        ]);
    let matching_many = loader.matching_rulesets(&ctx_many);

    assert_eq!(
        matching_few.len(),
        matching_many.len(),
        "RuleSet count should not change based on file count"
    );
}

// ============================================================================
// Prompt Rendering Tests — changed files and diff blocks
// ============================================================================

/// Test that ValidatorRenderContext::render() includes changed file paths
/// and fenced diff blocks in the rendered prompt sent to the LLM.
///
/// This directly tests the rendering pipeline rather than output format,
/// verifying that the context the validator agent actually sees contains
/// the right information.
#[test]
fn test_stop_validator_prompt_contains_changed_files_and_diffs() {
    use avp_common::turn::DIFF_TEXT_KEY;
    use avp_common::validator::{Severity, Validator, ValidatorFrontmatter, ValidatorSource};
    use std::path::PathBuf;
    use swissarmyhammer_prompts::{PromptLibrary, PromptResolver};

    // 1. Build a prompt library with builtin templates
    let mut prompt_library = PromptLibrary::new();
    let mut resolver = PromptResolver::new();
    resolver
        .load_all_prompts(&mut prompt_library)
        .expect("Should load builtin prompts");

    // 2. Create a test validator (simulating a Stop-triggered code-quality check)
    let validator = Validator {
        frontmatter: ValidatorFrontmatter {
            name: "test-code-quality".to_string(),
            description: "Test code quality validator".to_string(),
            severity: Severity::Error,
            trigger: HookType::Stop,
            match_criteria: None,
            trigger_matcher: None,
            tags: vec![],
            once: false,
            timeout: 30,
        },
        body: "Review the changed code for quality issues.".to_string(),
        source: ValidatorSource::Builtin,
        path: PathBuf::from("test-code-quality.md"),
    };

    // 3. Build a hook context with _diff_text (as prepare_validator_context produces)
    let diff_content =
        "```diff\n--- src/main.rs\n+++ src/main.rs\n@@ -1,3 +1,3 @@\n-fn old() {}\n+fn new() {}\n```\n";
    let mut hook_context = serde_json::json!({
        "session_id": "test-session",
        "cwd": "/project",
        "hook_event_name": "Stop",
        "stop_hook_active": true
    });
    hook_context
        .as_object_mut()
        .unwrap()
        .insert(DIFF_TEXT_KEY.to_string(), serde_json::json!(diff_content));

    // 4. Set up changed files list
    let changed_files = vec!["src/main.rs".to_string(), "tests/test_main.rs".to_string()];

    // 5. Render the prompt via ValidatorRenderContext
    let rendered =
        ValidatorRenderContext::new(&prompt_library, &validator, HookType::Stop, &hook_context)
            .with_changed_files(Some(&changed_files))
            .render()
            .expect("Prompt rendering should succeed");

    // 6. Assert changed file paths appear in the rendered prompt
    assert!(
        rendered.contains("src/main.rs"),
        "Rendered prompt should contain changed file path 'src/main.rs'.\nRendered:\n{}",
        rendered
    );
    assert!(
        rendered.contains("tests/test_main.rs"),
        "Rendered prompt should contain changed file path 'tests/test_main.rs'.\nRendered:\n{}",
        rendered
    );

    // 7. Assert the "Files Changed This Turn" section is present
    assert!(
        rendered.contains("Files Changed This Turn"),
        "Rendered prompt should contain 'Files Changed This Turn' section.\nRendered:\n{}",
        rendered
    );

    // 8. Assert fenced diff blocks appear in the rendered prompt
    assert!(
        rendered.contains("```diff"),
        "Rendered prompt should contain fenced diff block (```diff).\nRendered:\n{}",
        rendered
    );

    // 9. Assert the actual diff content is present
    assert!(
        rendered.contains("-fn old() {}"),
        "Rendered prompt should contain the removed line from the diff.\nRendered:\n{}",
        rendered
    );
    assert!(
        rendered.contains("+fn new() {}"),
        "Rendered prompt should contain the added line from the diff.\nRendered:\n{}",
        rendered
    );

    // 10. Assert the validator body is included
    assert!(
        rendered.contains("Review the changed code for quality issues."),
        "Rendered prompt should contain the validator body.\nRendered:\n{}",
        rendered
    );
}

// ============================================================================
// Session-Scoped Diff Isolation Tests
// ============================================================================

/// Integration test: two sessions write diffs through chain links and each sees
/// only its own diffs. Cleanup of one session does not affect the other.
///
/// This proves that subagent lifecycle (write + cleanup) is fully isolated from
/// the parent session's state and sidecar diff files.
#[tokio::test]
async fn test_session_scoped_diffs_isolated_through_chain_links() {
    let temp = TempDir::new().unwrap();
    let turn_state = Arc::new(TurnStateManager::new(temp.path()));

    // Create two files: a.rs for the parent session, b.rs for the subagent session
    let file_a = temp.path().join("a.rs");
    let file_b = temp.path().join("b.rs");
    fs::write(&file_a, "fn parent() {}\n").unwrap();
    fs::write(&file_b, "fn subagent() {}\n").unwrap();

    let pre_tracker = PreToolUseFileTracker::new(turn_state.clone());
    let post_tracker = PostToolUseFileTracker::new(turn_state.clone());

    // ── Parent session: PreToolUse → modify a.rs → PostToolUse ──────────
    let pre_input_a = HookInputBuilder::pre_tool_use_input(
        "parent-session",
        "Edit",
        &file_a.to_string_lossy(),
        "tool-a",
    );
    let mut ctx_a = ChainContext::new();
    pre_tracker.process(&pre_input_a, &mut ctx_a).await;

    fs::write(&file_a, "fn parent() { /* changed */ }\n").unwrap();

    let post_input_a = HookInputBuilder::post_tool_use_input(
        "parent-session",
        "Edit",
        &file_a.to_string_lossy(),
        "tool-a",
    );
    post_tracker.process(&post_input_a, &mut ctx_a).await;

    // ── Subagent session: PreToolUse → modify b.rs → PostToolUse ────────
    let pre_input_b = HookInputBuilder::pre_tool_use_input(
        "subagent-session",
        "Edit",
        &file_b.to_string_lossy(),
        "tool-b",
    );
    let mut ctx_b = ChainContext::new();
    pre_tracker.process(&pre_input_b, &mut ctx_b).await;

    fs::write(&file_b, "fn subagent() { /* changed */ }\n").unwrap();

    let post_input_b = HookInputBuilder::post_tool_use_input(
        "subagent-session",
        "Edit",
        &file_b.to_string_lossy(),
        "tool-b",
    );
    post_tracker.process(&post_input_b, &mut ctx_b).await;

    // ── Verify: each session's turn state has only its own changed file ──
    let parent_state = turn_state.load("parent-session").unwrap();
    assert_eq!(
        parent_state.changed.len(),
        1,
        "Parent session should have exactly 1 changed file"
    );
    assert_eq!(parent_state.changed[0], file_a);

    let subagent_state = turn_state.load("subagent-session").unwrap();
    assert_eq!(
        subagent_state.changed.len(),
        1,
        "Subagent session should have exactly 1 changed file"
    );
    assert_eq!(subagent_state.changed[0], file_b);

    // ── Verify: sidecar diffs are session-scoped ────────────────────────
    let parent_diffs = turn_state.load_all_diffs("parent-session");
    assert_eq!(
        parent_diffs.len(),
        1,
        "Parent session should have exactly 1 diff file"
    );
    let parent_diff_path = parent_diffs.keys().next().unwrap();
    assert!(
        parent_diff_path.contains("a.rs"),
        "Parent diff should be for a.rs, got: {}",
        parent_diff_path
    );

    let subagent_diffs = turn_state.load_all_diffs("subagent-session");
    assert_eq!(
        subagent_diffs.len(),
        1,
        "Subagent session should have exactly 1 diff file"
    );
    let subagent_diff_path = subagent_diffs.keys().next().unwrap();
    assert!(
        subagent_diff_path.contains("b.rs"),
        "Subagent diff should be for b.rs, got: {}",
        subagent_diff_path
    );

    // ── Verify: diff content is correct ─────────────────────────────────
    let parent_diff_text = parent_diffs.values().next().unwrap();
    assert!(
        parent_diff_text.contains("parent"),
        "Parent diff should reference parent function"
    );

    let subagent_diff_text = subagent_diffs.values().next().unwrap();
    assert!(
        subagent_diff_text.contains("subagent"),
        "Subagent diff should reference subagent function"
    );

    // ── Cleanup subagent session via SessionStartCleanup ────────────────
    let cleanup = SessionStartCleanup::new(turn_state.clone());
    let cleanup_input = SessionStartInput {
        common: HookInputBuilder::common_input("subagent-session", HookType::SessionStart),
        source: None,
        model: None,
    };
    let mut cleanup_ctx = ChainContext::new();
    cleanup.process(&cleanup_input, &mut cleanup_ctx).await;

    // ── Verify: subagent state is gone ──────────────────────────────────
    let subagent_state_after = turn_state.load("subagent-session").unwrap();
    assert!(
        subagent_state_after.changed.is_empty(),
        "Subagent session should be cleared after cleanup"
    );
    let subagent_diffs_after = turn_state.load_all_diffs("subagent-session");
    assert!(
        subagent_diffs_after.is_empty(),
        "Subagent diffs should be cleared after cleanup"
    );

    // ── Verify: parent state is STILL intact ────────────────────────────
    let parent_state_after = turn_state.load("parent-session").unwrap();
    assert_eq!(
        parent_state_after.changed.len(),
        1,
        "Parent session should still have 1 changed file after subagent cleanup"
    );
    assert_eq!(parent_state_after.changed[0], file_a);

    let parent_diffs_after = turn_state.load_all_diffs("parent-session");
    assert_eq!(
        parent_diffs_after.len(),
        1,
        "Parent diffs should still be intact after subagent cleanup"
    );
    assert!(
        parent_diffs_after.keys().next().unwrap().contains("a.rs"),
        "Parent diff should still be for a.rs after subagent cleanup"
    );
}

// ============================================================================
// Per-RuleSet Diff Filtering Integration Tests
// ============================================================================

/// Helper: Create a Stop RuleSet on disk with specific match.files patterns.
///
/// Writes a VALIDATOR.md with the given name, file patterns, and a minimal rule
/// into the `validators/<name>` subdirectory of `base`.
fn create_stop_ruleset_with_file_patterns(
    base: &std::path::Path,
    name: &str,
    file_patterns: &[&str],
) {
    let ruleset_dir = base.join("validators").join(name);
    fs::create_dir_all(ruleset_dir.join("rules")).unwrap();

    // Build the YAML files list
    let files_yaml: String = file_patterns
        .iter()
        .map(|p| format!("    - \"{}\"", p))
        .collect::<Vec<_>>()
        .join("\n");

    let manifest = format!(
        "---\nname: {name}\ndescription: Test RuleSet for {name}\nversion: 1.0.0\ntrigger: Stop\nmatch:\n  files:\n{files_yaml}\nseverity: error\ntimeout: 30\n---\n\n# {name} RuleSet\n\nTest RuleSet.\n",
    );

    fs::write(ruleset_dir.join("VALIDATOR.md"), manifest).unwrap();

    fs::write(
        ruleset_dir.join("rules").join("check.md"),
        format!(
            "---\nname: {name}-check\ndescription: Check rule for {name}\n---\n\n# Check\n\nValidation instructions.\n",
        ),
    )
    .unwrap();
}

/// Integration test: two Stop RuleSets with different match.files patterns each
/// receive only the diffs for files matching their patterns.
///
/// This is an end-to-end test that:
/// 1. Creates two RuleSets on disk (`rust-only` with `*.rs`, `python-only` with `*.py`)
/// 2. Loads them via `ValidatorLoader::load_rulesets_directory` (real parsing)
/// 3. Writes sidecar diffs for both `.rs` and `.py` files
/// 4. Calls `filter_diffs_for_ruleset` for each loaded RuleSet
/// 5. Asserts each RuleSet sees only its matching diffs
#[test]
fn test_stop_rulesets_receive_only_matching_diffs() {
    use avp_common::turn::FileDiff;
    use avp_common::validator::runner::filter_diffs_for_ruleset;
    use avp_common::validator::ValidatorSource;
    use std::path::PathBuf;

    let temp = TempDir::new().unwrap();

    // 1. Create two RuleSets on disk with different file patterns
    create_stop_ruleset_with_file_patterns(temp.path(), "rust-only", &["*.rs"]);
    create_stop_ruleset_with_file_patterns(temp.path(), "python-only", &["*.py"]);

    // 2. Load them via ValidatorLoader (real RuleSet loading, not mocked)
    let mut loader = ValidatorLoader::new();
    loader
        .load_rulesets_directory(&temp.path().join("validators"), ValidatorSource::Project)
        .expect("Should load test RuleSets from disk");

    let rust_ruleset = loader
        .get_ruleset("rust-only")
        .expect("rust-only RuleSet should be loaded");
    let python_ruleset = loader
        .get_ruleset("python-only")
        .expect("python-only RuleSet should be loaded");

    // Verify the loaded match criteria are correct
    let rust_files = &rust_ruleset
        .manifest
        .match_criteria
        .as_ref()
        .expect("rust-only should have match criteria")
        .files;
    assert_eq!(rust_files, &["*.rs"], "rust-only should match *.rs");

    let python_files = &python_ruleset
        .manifest
        .match_criteria
        .as_ref()
        .expect("python-only should have match criteria")
        .files;
    assert_eq!(python_files, &["*.py"], "python-only should match *.py");

    // 3. Write sidecar diffs for both file types via TurnStateManager
    let turn_state = TurnStateManager::new(temp.path());
    let session_id = "diff-filter-session";

    let rs_diff = "--- src/main.rs\n+++ src/main.rs\n@@ -1 +1 @@\n-fn old() {}\n+fn new() {}\n";
    let py_diff = "--- lib/helper.py\n+++ lib/helper.py\n@@ -1 +1 @@\n-def old():\n+def new():\n";

    turn_state
        .write_diff(session_id, &PathBuf::from("src/main.rs"), rs_diff)
        .expect("Should write .rs diff");
    turn_state
        .write_diff(session_id, &PathBuf::from("lib/helper.py"), py_diff)
        .expect("Should write .py diff");

    // 4. Build FileDiff structs (as the Stop chain would construct them)
    let diffs = vec![
        FileDiff {
            path: PathBuf::from("src/main.rs"),
            diff_text: rs_diff.to_string(),
            is_new_file: false,
            is_binary: false,
        },
        FileDiff {
            path: PathBuf::from("lib/helper.py"),
            diff_text: py_diff.to_string(),
            is_new_file: false,
            is_binary: false,
        },
    ];

    // 5. Filter diffs for each RuleSet and assert isolation
    let rust_diffs = filter_diffs_for_ruleset(Some(&diffs), rust_ruleset)
        .expect("filter should return Some for rust-only");
    let python_diffs = filter_diffs_for_ruleset(Some(&diffs), python_ruleset)
        .expect("filter should return Some for python-only");

    // rust-only sees only .rs diffs
    let rust_paths: Vec<String> = rust_diffs
        .iter()
        .map(|d| d.path.display().to_string())
        .collect();
    assert_eq!(
        rust_paths,
        vec!["src/main.rs"],
        "rust-only RuleSet should see only .rs diffs, got: {:?}",
        rust_paths
    );
    assert!(
        rust_diffs[0].diff_text.contains("-fn old()"),
        "rust-only diff should contain the Rust diff content"
    );

    // python-only sees only .py diffs
    let python_paths: Vec<String> = python_diffs
        .iter()
        .map(|d| d.path.display().to_string())
        .collect();
    assert_eq!(
        python_paths,
        vec!["lib/helper.py"],
        "python-only RuleSet should see only .py diffs, got: {:?}",
        python_paths
    );
    assert!(
        python_diffs[0].diff_text.contains("-def old()"),
        "python-only diff should contain the Python diff content"
    );

    // Cross-check: rust-only does NOT see .py diffs
    assert!(
        !rust_paths.iter().any(|p| p.ends_with(".py")),
        "rust-only RuleSet must NOT see .py diffs"
    );

    // Cross-check: python-only does NOT see .rs diffs
    assert!(
        !python_paths.iter().any(|p| p.ends_with(".rs")),
        "python-only RuleSet must NOT see .rs diffs"
    );
}
