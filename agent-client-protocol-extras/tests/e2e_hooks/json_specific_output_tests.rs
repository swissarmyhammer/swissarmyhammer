//! hookSpecificOutput JSON tests.
//!
//! Tests for event-specific output fields:
//! - `additionalContext` per event type → AllowWithContext
//! - PreToolUse `permissionDecision: "deny"` → Block
//! - PreToolUse `updatedInput` → AllowWithUpdatedInput
//! - Stop `reason` → ShouldContinue

use agent_client_protocol::Agent;
use tokio::sync::broadcast;

use crate::helpers;
use std::sync::Arc;

/// Maximum time to wait for an async channel message in notification tests.
const CHANNEL_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

/// PreToolUse hookSpecificOutput.additionalContext → AllowWithContext via context channel.
#[tokio::test]
async fn pre_tool_use_additional_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"extra info from hook"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, mut context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    let ctx = tokio::time::timeout(CHANNEL_TIMEOUT, context_rx.recv()).await;
    assert!(
        ctx.is_ok(),
        "PreToolUse additionalContext should deliver via context channel"
    );
    assert!(
        ctx.unwrap().unwrap().contains("extra info from hook"),
        "Context should contain the additionalContext value"
    );
}

/// UserPromptSubmit hookSpecificOutput.additionalContext → AllowWithContext → context injected.
#[tokio::test]
async fn user_prompt_submit_additional_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"extra info"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("UserPromptSubmit", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;

    // AllowWithContext → prompt succeeds with context injected
    let result = agent
        .prompt(helpers::make_prompt_request(session_id, "hello"))
        .await;
    assert!(
        result.is_ok(),
        "UserPromptSubmit AllowWithContext should allow prompt, got: {:?}",
        result.err()
    );
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "UserPromptSubmit hook should have been invoked"
    );
}

/// PostToolUse hookSpecificOutput.additionalContext → AllowWithContext via context channel.
#[tokio::test]
async fn post_tool_use_additional_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"post-tool context"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PostToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, mut context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    let ctx = tokio::time::timeout(CHANNEL_TIMEOUT, context_rx.recv()).await;
    assert!(
        ctx.is_ok(),
        "PostToolUse additionalContext should deliver via context channel"
    );
    assert!(
        ctx.unwrap().unwrap().contains("post-tool context"),
        "Context should contain the additionalContext value"
    );
}

/// PostToolUseFailure hookSpecificOutput.additionalContext → AllowWithContext via context channel.
#[tokio::test]
async fn post_tool_use_failure_additional_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"hookSpecificOutput":{"hookEventName":"PostToolUseFailure","additionalContext":"failure context"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json =
        helpers::hook_config_json("PostToolUseFailure", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, mut context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_failed_notifications(&tx, "test-session").await;

    let ctx = tokio::time::timeout(CHANNEL_TIMEOUT, context_rx.recv()).await;
    assert!(
        ctx.is_ok(),
        "PostToolUseFailure additionalContext should deliver via context channel"
    );
    assert!(
        ctx.unwrap().unwrap().contains("failure context"),
        "Context should contain the additionalContext value"
    );
}

/// SessionStart hookSpecificOutput.additionalContext → AllowWithContext, silently ignored.
#[tokio::test]
async fn session_start_additional_context_ignored() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"session context"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("SessionStart", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    // SessionStart decisions are discarded — session should still be created
    let _session_id = helpers::init_session(&agent).await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "SessionStart hook should have been invoked"
    );
}

/// Notification hookSpecificOutput.additionalContext → AllowWithContext via context channel.
#[tokio::test]
async fn notification_additional_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"hookSpecificOutput":{"hookEventName":"Notification","additionalContext":"notification context"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Notification", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, mut context_rx) = agent.intercept_notifications(rx);

    helpers::send_agent_message_notification(&tx, "test-session").await;

    let ctx = tokio::time::timeout(CHANNEL_TIMEOUT, context_rx.recv()).await;
    assert!(
        ctx.is_ok(),
        "Notification additionalContext should deliver via context channel"
    );
    assert!(
        ctx.unwrap().unwrap().contains("notification context"),
        "Context should contain the additionalContext value"
    );
}

/// Stop hookSpecificOutput.reason → ShouldContinue with reason in meta.
#[tokio::test]
async fn stop_specific_reason_should_continue() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output =
        r#"{"hookSpecificOutput":{"hookEventName":"Stop","reason":"keep going please"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("Stop", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let response = helpers::run_prompt(&agent, &session_id, "Run a bash command").await;

    let meta = response.meta.unwrap_or_default();
    let should_continue = meta
        .get("hook_should_continue")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(
        should_continue,
        "Stop hookSpecificOutput.reason should set hook_should_continue in meta"
    );
    let hook_reason = meta
        .get("hook_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(
        hook_reason.contains("keep going please"),
        "Meta hook_reason should contain the reason, got: {}",
        hook_reason
    );
}

/// PreToolUse hookSpecificOutput.permissionDecision:deny → Block (no-op in notification pipeline).
#[tokio::test]
async fn pre_tool_use_permission_decision_deny() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"tool not permitted"}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    // Block is silently ignored in notification pipeline (tool already initiated).
    // Verify hook ran.
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "PreToolUse hook should have been invoked"
    );
    let json: serde_json::Value =
        serde_json::from_str(&captured.unwrap()).expect("Captured stdin should be valid JSON");
    assert_eq!(json["hook_event_name"], "PreToolUse");
}

/// PreToolUse hookSpecificOutput.updatedInput → AllowWithUpdatedInput (logged as Allow in pipeline).
#[tokio::test]
async fn pre_tool_use_updated_input() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output =
        r#"{"hookSpecificOutput":{"hookEventName":"PreToolUse","updatedInput":{"modified":true}}}"#;
    let script = helpers::write_json_output_script(tmp.path(), "hook.sh", json_output);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_tool_completed_notifications(&tx, "test-session").await;

    // AllowWithUpdatedInput is logged and treated as Allow in notification pipeline.
    // Verify hook ran.
    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "PreToolUse hook should have been invoked"
    );
    let json: serde_json::Value =
        serde_json::from_str(&captured.unwrap()).expect("Captured stdin should be valid JSON");
    assert_eq!(json["hook_event_name"], "PreToolUse");
}
