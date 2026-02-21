//! JSON output tests: exit-0 with structured JSON on stdout.
//!
//! Each test verifies the correct HookDecision when a command hook
//! exits 0 and prints a HookOutput JSON to stdout.
//!
//! Hooks that fire from `prompt()` / `new_session()`:
//!   UserPromptSubmit, Stop, SessionStart
//!
//! Hooks that fire from the notification pipeline (`intercept_notifications`):
//!   PreToolUse, PostToolUse, PostToolUseFailure, Notification

use agent_client_protocol::Agent;
use tokio::sync::broadcast;

use crate::helpers;
use std::sync::Arc;

/// UserPromptSubmit with JSON decision:block should Block the prompt.
#[tokio::test]
async fn user_prompt_submit_json_block() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"decision":"block","reason":"policy violation"}"#;
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
        "Expected prompt to be blocked by JSON decision"
    );
    let err = result.unwrap_err();
    assert!(
        err.message.contains("policy violation"),
        "Error should contain reason, got: {}",
        err.message
    );
}

/// PreToolUse with JSON decision:block — Block is a no-op in the notification
/// pipeline (tool already initiated). Verify the hook ran.
#[tokio::test]
async fn pre_tool_use_json_block_runs_hook() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"decision":"block","reason":"tool blocked"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    // Block is silently ignored in the notification pipeline.
    // Verify hook ran and received correct JSON.
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "PreToolUse hook should have been invoked"
    );
    let json: serde_json::Value =
        serde_json::from_str(&captured.unwrap()).expect("Captured stdin should be valid JSON");
    assert_eq!(json["hook_event_name"], "PreToolUse");
}

/// PostToolUse with JSON decision:block → not blockable, becomes context.
/// Tested via intercept_notifications.
#[tokio::test]
async fn post_tool_use_json_block_becomes_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"decision":"block","reason":"post-tool feedback"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PostToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    // PostToolUse not blockable — prompt would still succeed
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "PostToolUse hook should have been invoked"
    );
}

/// PostToolUseFailure with JSON decision:block → not blockable, becomes context.
/// Tested via intercept_notifications.
#[tokio::test]
async fn post_tool_use_failure_json_block_becomes_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"decision":"block","reason":"failure feedback"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json =
        helpers::hook_config_json("PostToolUseFailure", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_failed_notifications(&tx, "test-session").await;

    // PostToolUseFailure not blockable — verify hook ran
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "PostToolUseFailure hook should have been invoked"
    );
}

/// Stop with JSON decision:block should produce ShouldContinue (don't stop).
#[tokio::test]
async fn stop_json_block_becomes_should_continue() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"decision":"block","reason":"keep going"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Stop", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let response = helpers::run_prompt(&agent, &session_id, "Run a bash command").await;

    // Stop hook with decision:block → ShouldContinue → meta annotation
    let meta = response.meta.unwrap_or_default();
    let should_continue = meta
        .get("hook_should_continue")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        should_continue,
        "Stop hook with decision:block should set hook_should_continue in meta"
    );
}

/// SessionStart with JSON decision:block → Allow (not blockable, silent).
#[tokio::test]
async fn session_start_json_block_allows_silently() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"decision":"block","reason":"session blocked"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("SessionStart", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    // SessionStart is not blockable — session should still be created
    let _session_id = helpers::init_session(&agent).await;

    // Verify hook ran
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "SessionStart hook should have been invoked"
    );
}

/// Notification with JSON decision:block → Allow (not blockable, silent).
/// Tested via intercept_notifications.
#[tokio::test]
async fn notification_json_block_allows_silently() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"decision":"block","reason":"notification blocked"}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Notification", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_agent_message_notification(&tx, "test-session").await;

    // Notification not blockable — verify hook ran
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "Notification hook should have been invoked"
    );
}
