//! JSON `continue: false` tests: one per HookEventKind variant.
//!
//! Each test verifies the Cancel decision when a command hook exits 0 and
//! prints `{"continue": false, "stopReason": "..."}` to stdout.
//!
//! Hooks that fire from `prompt()` / `new_session()`:
//!   UserPromptSubmit → Cancel → prompt error
//!   Stop → Cancel → silently ignored (only ShouldContinue checked)
//!   SessionStart → Cancel → silently ignored (decisions discarded)
//!
//! Hooks that fire from the notification pipeline (`intercept_notifications`):
//!   PreToolUse, PostToolUse, PostToolUseFailure, Notification
//!   → Cancel → cancel channel receives session ID

use agent_client_protocol::Agent;
use tokio::sync::broadcast;

use crate::helpers;
use std::sync::Arc;

/// Maximum time to wait for an async channel message in notification tests.
const CHANNEL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// UserPromptSubmit with continue:false should cancel the prompt.
#[tokio::test]
async fn user_prompt_submit_continue_false_cancels() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"continue":false,"stopReason":"hook requested stop"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("UserPromptSubmit", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = agent
        .prompt(helpers::make_prompt_request(session_id, "hello"))
        .await;

    assert!(
        result.is_err(),
        "Expected prompt to be cancelled by continue:false"
    );
    let err = result.unwrap_err();
    assert!(
        err.message.contains("hook requested stop"),
        "Error should contain stopReason, got: {}",
        err.message
    );
}

/// PreToolUse with continue:false should send Cancel to the cancel channel.
#[tokio::test]
async fn pre_tool_use_continue_false_cancels() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"continue":false,"stopReason":"hook requested stop"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, mut cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    let cancel = tokio::time::timeout(CHANNEL_TIMEOUT, cancel_rx.recv()).await;
    assert!(
        cancel.is_ok(),
        "PreToolUse continue:false should send Cancel to cancel channel"
    );
}

/// PostToolUse with continue:false should send Cancel to the cancel channel.
#[tokio::test]
async fn post_tool_use_continue_false_cancels() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"continue":false,"stopReason":"hook requested stop"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PostToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, mut cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    let cancel = tokio::time::timeout(CHANNEL_TIMEOUT, cancel_rx.recv()).await;
    assert!(
        cancel.is_ok(),
        "PostToolUse continue:false should send Cancel to cancel channel"
    );
}

/// PostToolUseFailure with continue:false should send Cancel to the cancel channel.
#[tokio::test]
async fn post_tool_use_failure_continue_false_cancels() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"continue":false,"stopReason":"hook requested stop"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json =
        helpers::hook_config_json("PostToolUseFailure", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, mut cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_failed_notifications(&tx, "test-session").await;

    let cancel = tokio::time::timeout(CHANNEL_TIMEOUT, cancel_rx.recv()).await;
    assert!(
        cancel.is_ok(),
        "PostToolUseFailure continue:false should send Cancel to cancel channel"
    );
}

/// Stop with continue:false produces Cancel, but the prompt path only checks
/// for ShouldContinue — Cancel is silently ignored.
#[tokio::test]
async fn stop_continue_false_ignored() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"continue":false,"stopReason":"hook requested stop"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Stop", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let response = helpers::run_prompt(&agent, &session_id, "Run a bash command").await;

    // Stop hook with Cancel is silently ignored — no ShouldContinue annotation
    let meta = response.meta.unwrap_or_default();
    let should_continue = meta
        .get("hook_should_continue")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        !should_continue,
        "Stop hook with Cancel should NOT set hook_should_continue"
    );
}

/// SessionStart with continue:false produces Cancel, but fire_session_start
/// discards all decisions — session should still be created.
#[tokio::test]
async fn session_start_continue_false_ignored() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"continue":false,"stopReason":"hook requested stop"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("SessionStart", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    // SessionStart fires inside new_session — Cancel should be silently ignored
    let _session_id = helpers::init_session(&agent).await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "SessionStart hook should have been invoked"
    );
}

/// Notification with continue:false should send Cancel to the cancel channel.
#[tokio::test]
async fn notification_continue_false_cancels() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"continue":false,"stopReason":"hook requested stop"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Notification", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, mut cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_agent_message_notification(&tx, "test-session").await;

    let cancel = tokio::time::timeout(CHANNEL_TIMEOUT, cancel_rx.recv()).await;
    assert!(
        cancel.is_ok(),
        "Notification continue:false should send Cancel to cancel channel"
    );
}
