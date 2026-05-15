//! Edge-case hook tests covering integration scenarios missed by
//! the per-event-kind exit-code / JSON-output / specific-output suites:
//!
//! - Prompt and agent evaluator hooks (e2e, not just unit tests)
//! - Multiple command hooks on the same event
//! - Malformed JSON output from command hooks
//! - Nonexistent hook command (spawn failure)
//! - SessionStart matcher filtering by source
//! - Unexpected exit codes on notification-pipeline events

use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::helpers;

// ---------------------------------------------------------------------------
// Prompt / agent evaluator hooks
// ---------------------------------------------------------------------------

/// A prompt evaluator returning `{"ok": true}` should allow the prompt.
#[tokio::test]
async fn prompt_evaluator_allows_on_ok_true() {
    let playback = helpers::load_playback_agent("tool_call_session.json");
    let evaluator = helpers::MockEvaluator::allowing();
    let config_json = r#"{
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{ "type": "prompt", "prompt": "Evaluate: $ARGUMENTS" }]
            }]
        }
    }"#;
    let agent =
        helpers::build_hookable_agent_with_evaluator(playback, config_json, Arc::new(evaluator));

    let session_id = helpers::init_session(&agent).await;
    let result = helpers::try_run_prompt(&agent, &session_id, "test").await;

    assert!(
        result.is_ok(),
        "Prompt evaluator ok=true should allow, got: {:?}",
        result.err()
    );
}

/// A prompt evaluator returning `{"ok": false, "reason": "..."}` should block.
#[tokio::test]
async fn prompt_evaluator_blocks_on_ok_false() {
    let playback = helpers::load_playback_agent("tool_call_session.json");
    let evaluator = helpers::MockEvaluator::blocking("unsafe prompt");
    let config_json = r#"{
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{ "type": "prompt", "prompt": "Evaluate: $ARGUMENTS" }]
            }]
        }
    }"#;
    let agent =
        helpers::build_hookable_agent_with_evaluator(playback, config_json, Arc::new(evaluator));

    let session_id = helpers::init_session(&agent).await;
    let result = helpers::try_run_prompt(&agent, &session_id, "test").await;

    assert!(result.is_err(), "Prompt evaluator ok=false should block");
    let err = result.unwrap_err();
    assert!(
        err.message.contains("unsafe prompt"),
        "Error should contain reason, got: {}",
        err.message
    );
}

/// An agent evaluator should be called with `is_agent=true`.
#[tokio::test]
async fn agent_evaluator_passes_is_agent_flag() {
    let playback = helpers::load_playback_agent("tool_call_session.json");
    let (evaluator, is_agent_flag) = helpers::MockEvaluator::with_agent_tracking();
    let config_json = r#"{
        "hooks": {
            "UserPromptSubmit": [{
                "hooks": [{ "type": "agent", "prompt": "Check: $ARGUMENTS" }]
            }]
        }
    }"#;
    let agent =
        helpers::build_hookable_agent_with_evaluator(playback, config_json, Arc::new(evaluator));

    let session_id = helpers::init_session(&agent).await;
    let result = helpers::try_run_prompt(&agent, &session_id, "test").await;

    assert!(
        result.is_ok(),
        "Agent evaluator should allow, got: {:?}",
        result.err()
    );
    assert!(
        is_agent_flag.load(Ordering::SeqCst),
        "Evaluator should have been called with is_agent=true"
    );
}

// ---------------------------------------------------------------------------
// Multiple command hooks
// ---------------------------------------------------------------------------

/// Two command hooks on the same event should both fire.
#[tokio::test]
async fn multiple_command_hooks_both_fire() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script1 = helpers::write_exit_script(tmp.path(), "hook1.sh", 0, "");
    let script2 = helpers::write_exit_script(tmp.path(), "hook2.sh", 0, "");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_two_hooks_json(
        "UserPromptSubmit",
        script1.to_str().unwrap(),
        script2.to_str().unwrap(),
    );
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let session_id = helpers::init_session(&agent).await;
    let _ = helpers::run_prompt(&agent, &session_id, "hello").await;

    assert!(
        helpers::read_stdin_capture(tmp.path(), "hook1.sh").is_some(),
        "First hook should fire"
    );
    assert!(
        helpers::read_stdin_capture(tmp.path(), "hook2.sh").is_some(),
        "Second hook should fire"
    );
}

/// When one hook allows (exit 0) and another blocks (exit 2), Block wins.
#[tokio::test]
async fn multiple_hooks_block_takes_precedence() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script1 = helpers::write_exit_script(tmp.path(), "hook1.sh", 0, "");
    let script2 = helpers::write_exit_script(tmp.path(), "hook2.sh", 2, "blocked by second hook");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_two_hooks_json(
        "UserPromptSubmit",
        script1.to_str().unwrap(),
        script2.to_str().unwrap(),
    );
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = helpers::try_run_prompt(&agent, &session_id, "hello").await;

    assert!(result.is_err(), "Block should take precedence over Allow");
    let err = result.unwrap_err();
    assert!(
        err.message.contains("blocked by second hook"),
        "Error should contain block reason, got: {}",
        err.message
    );

    // Both hooks should have been invoked
    assert!(
        helpers::read_stdin_capture(tmp.path(), "hook1.sh").is_some(),
        "First hook should fire"
    );
    assert!(
        helpers::read_stdin_capture(tmp.path(), "hook2.sh").is_some(),
        "Second hook should fire"
    );
}

// ---------------------------------------------------------------------------
// Malformed output / nonexistent command
// ---------------------------------------------------------------------------

/// A hook that prints non-JSON to stdout and exits 0 should be treated as Allow.
#[tokio::test]
async fn malformed_json_output_treated_as_allow() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_malformed_output_script(tmp.path(), "hook.sh");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("UserPromptSubmit", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = helpers::try_run_prompt(&agent, &session_id, "hello").await;

    assert!(
        result.is_ok(),
        "Malformed JSON output should be treated as Allow, got: {:?}",
        result.err()
    );
}

/// A hook pointing to a nonexistent command should be treated as Allow
/// (spawn failure → fallback to Allow).
#[tokio::test]
async fn nonexistent_command_treated_as_allow() {
    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json =
        helpers::hook_config_json("UserPromptSubmit", "/nonexistent/path/to/hook", None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = helpers::try_run_prompt(&agent, &session_id, "hello").await;

    assert!(
        result.is_ok(),
        "Nonexistent command should be treated as Allow, got: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// SessionStart matcher
// ---------------------------------------------------------------------------

/// SessionStart matcher `"^startup$"` should fire for new_session (source="startup")
/// but NOT for load_session (source="resume").
#[tokio::test]
async fn session_start_matcher_filters_by_source() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 0, "");

    let playback = helpers::load_playback_agent("session_with_load.json");
    let config_json =
        helpers::hook_config_json("SessionStart", script.to_str().unwrap(), Some("^startup$"));
    let agent = helpers::build_hookable_agent(playback, &config_json);

    // new_session → SessionStart(source="startup") → matches "^startup$" → fires
    let _session_id = helpers::init_session(&agent).await;
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "SessionStart hook should fire for source='startup'"
    );

    // Clear capture for the next check
    helpers::clear_stdin_capture(tmp.path(), "hook.sh");

    // load_session → SessionStart(source="resume") → does NOT match "^startup$"
    let _ = helpers::resume_session(&agent, "hook-test-session").await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_none(),
        "SessionStart hook should NOT fire for source='resume'"
    );
}

// ---------------------------------------------------------------------------
// Unexpected exit code on notification-pipeline event
// ---------------------------------------------------------------------------

/// Exit code 1 on PreToolUse should be treated as Allow — no cancel or context.
#[tokio::test]
async fn unexpected_exit_code_treated_as_allow_on_pre_tool_use() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_code_script(tmp.path(), "hook.sh", 1);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, mut cancel_rx, mut context_rx) = agent.intercept_notifications(rx);

    helpers::send_named_tool_notification(&tx, "test-session", "Bash", "call-1").await;

    // Hook should have fired
    let captured = helpers::wait_for_stdin_capture(tmp.path(), "hook.sh").await;
    assert!(captured.is_some(), "PreToolUse hook should fire");

    // Hook already finished (wait_for_stdin_capture confirmed it), so any
    // channel sends would have already occurred.  Use a short timeout for
    // negative assertions.
    let short = std::time::Duration::from_millis(200);
    let cancel = tokio::time::timeout(short, cancel_rx.recv()).await;
    assert!(
        cancel.is_err(),
        "Unexpected exit code should not produce cancel"
    );

    let ctx = tokio::time::timeout(short, context_rx.recv()).await;
    assert!(
        ctx.is_err(),
        "Unexpected exit code should not produce context"
    );
}

// ---------------------------------------------------------------------------
// Forward-compatible hook event config tests
// ---------------------------------------------------------------------------

/// New event types are accepted in config and registered as hooks, but
/// since no ACP lifecycle point fires them, they don't trigger during
/// a normal prompt flow.
#[tokio::test]
async fn new_event_hooks_registered_but_not_triggered_by_prompt() {
    // These event types have dedicated HookEventKind variants and are
    // properly registered, but HookableAgent's Agent trait impl doesn't
    // produce them — they can only be fired manually.
    let new_event_names = [
        "Elicitation",
        "ElicitationResult",
        "InstructionsLoaded",
        "ConfigChange",
        "WorktreeCreate",
        "WorktreeRemove",
    ];

    for event_name in &new_event_names {
        let tmp = tempfile::TempDir::new().unwrap();
        let script = helpers::write_exit_script(tmp.path(), "hook.sh", 0, "");
        let config = helpers::hook_config_json(event_name, script.to_str().unwrap(), None);
        let playback = helpers::load_playback_agent("tool_call_session.json");
        // Config is accepted — hooks are registered (not skipped)
        let agent = helpers::build_hookable_agent(playback, &config);
        let session_id = helpers::init_session(&agent).await;
        // Prompt succeeds — these hooks are registered but no ACP event triggers them
        let _response = helpers::run_prompt(&agent, &session_id, "test").await;
        // Hook should NOT have fired (no ACP lifecycle point produces these events)
        let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
        assert!(
            captured.is_none(),
            "{} hook should not fire during normal prompt flow",
            event_name
        );
    }
}

/// Mixed config with prompt-triggered and non-triggered events — prompt-path
/// hooks fire normally, new event hooks are registered but not triggered.
#[tokio::test]
async fn mixed_supported_and_forward_compat_config() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Supported hook that WILL fire
    let supported_script = helpers::write_exit_script(tmp.path(), "supported.sh", 0, "");
    // Forward-compat hook that should NOT fire
    let compat_script = helpers::write_exit_script(tmp.path(), "compat.sh", 0, "");

    let config = format!(
        r#"{{
            "hooks": {{
                "Stop": [{{
                    "hooks": [{{ "type": "command", "command": "{}" }}]
                }}],
                "Elicitation": [{{
                    "hooks": [{{ "type": "command", "command": "{}" }}]
                }}]
            }}
        }}"#,
        supported_script.to_str().unwrap(),
        compat_script.to_str().unwrap()
    );

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let agent = helpers::build_hookable_agent(playback, &config);
    let session_id = helpers::init_session(&agent).await;
    let _response = helpers::run_prompt(&agent, &session_id, "test").await;

    // Stop hook should have fired
    let supported_captured = helpers::read_stdin_capture(tmp.path(), "supported.sh");
    assert!(
        supported_captured.is_some(),
        "Supported Stop hook should fire"
    );

    // Elicitation hook should NOT have fired
    let compat_captured = helpers::read_stdin_capture(tmp.path(), "compat.sh");
    assert!(
        compat_captured.is_none(),
        "Forward-compat Elicitation hook should not fire"
    );
}
