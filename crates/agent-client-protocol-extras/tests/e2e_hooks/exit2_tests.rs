//! Exit-code-2 tests: one per HookEventKind variant.
//!
//! Each test verifies the correct HookDecision when a command hook
//! exits with code 2 and writes a message to stderr.
//!
//! Hooks that fire from `prompt()` / `new_session()`:
//!   UserPromptSubmit, Stop, SessionStart
//!
//! Hooks that fire from the notification pipeline (`intercept_notifications`):
//!   PreToolUse, PostToolUse, PostToolUseFailure, Notification

use agent_client_protocol_extras::PreToolUseOutcome;
use tokio::sync::broadcast;

use crate::helpers;

/// UserPromptSubmit is blockable — exit 2 should Block and reject the prompt.
#[tokio::test]
async fn user_prompt_submit_exit2_blocks() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 2, "blocked by policy");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("UserPromptSubmit", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = helpers::try_run_prompt(&agent, &session_id, "hello").await;

    // UserPromptSubmit Block → prompt returns an error
    assert!(result.is_err(), "Expected prompt to be blocked");
    let err = result.unwrap_err();
    assert!(
        err.message.contains("blocked by policy"),
        "Error should contain stderr message, got: {}",
        err.message
    );
}

/// PreToolUse is blockable — exit 2 produces Block, which at the dispatch seam
/// is a genuine Deny (the tool never runs).
#[tokio::test]
async fn pre_tool_use_exit2_denies() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 2, "tool not allowed");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let outcome = helpers::fire_pre_tool_use(&agent, "Bash").await;
    match outcome {
        PreToolUseOutcome::Deny { reason } => assert!(
            reason.contains("tool not allowed"),
            "exit-2 stderr should become the deny reason; got {reason:?}"
        ),
        other => panic!("expected Deny, got {other:?}"),
    }
}

/// PostToolUse is NOT blockable — exit 2 feeds stderr to the model as context,
/// surfaced after a successful call at the dispatch seam.
#[tokio::test]
async fn post_tool_use_exit2_feeds_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 2, "review this output");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PostToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let ctx = helpers::fire_post_tool_use(&agent, "Bash").await;
    assert!(
        ctx.as_deref()
            .unwrap_or_default()
            .contains("review this output"),
        "PostToolUse exit-2 should feed stderr as context; got {ctx:?}"
    );
}

/// PostToolUseFailure is NOT blockable — exit 2 feeds stderr to the model as
/// context, surfaced after a failed call at the dispatch seam.
#[tokio::test]
async fn post_tool_use_failure_exit2_feeds_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 2, "failure feedback");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json =
        helpers::hook_config_json("PostToolUseFailure", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let ctx = helpers::fire_post_tool_use_failure(&agent, "Bash").await;
    assert!(
        ctx.as_deref()
            .unwrap_or_default()
            .contains("failure feedback"),
        "PostToolUseFailure exit-2 should feed stderr as context; got {ctx:?}"
    );
}

/// SessionStart fires during new_session — exit 2 on a non-blockable,
/// non-post-tool event should Allow (silent warning only).
#[tokio::test]
async fn session_start_exit2_allows_silently() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 2, "session warning");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("SessionStart", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    // SessionStart fires inside new_session — should NOT error
    let _session_id = helpers::init_session(&agent).await;

    // Verify hook actually ran by checking stdin capture
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(captured.is_some(), "Hook script should have been invoked");
    let json: serde_json::Value =
        serde_json::from_str(&captured.unwrap()).expect("Captured stdin should be valid JSON");
    assert_eq!(json["hook_event_name"], "SessionStart");
}

/// Stop exit-2 should produce ShouldContinue (prevent stopping).
#[tokio::test]
async fn stop_exit2_should_continue() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 2, "keep going");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Stop", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let session_id = helpers::init_session(&agent).await;
    let response = helpers::run_prompt(&agent, &session_id, "Run a bash command").await;

    // Stop exit-2 → ShouldContinue → meta annotation
    let meta = response.meta.unwrap_or_default();
    let should_continue = meta
        .get("hook_should_continue")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        should_continue,
        "Stop hook with exit-2 should set hook_should_continue in meta"
    );
}

/// Notification exit-2 should Allow silently (non-blockable, non-post-tool).
/// Tested via intercept_notifications with a broadcast channel.
#[tokio::test]
async fn notification_exit2_allows_silently() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 2, "notification warning");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Notification", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(playback, &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_agent_message_notification(&tx, "test-session").await;

    // Notification exit-2 → Allow (silent warning) → no cancel, no context
    // Verify hook ran
    let captured = helpers::wait_for_stdin_capture(tmp.path(), "hook.sh").await;
    assert!(
        captured.is_some(),
        "Notification hook should have been invoked"
    );
}
