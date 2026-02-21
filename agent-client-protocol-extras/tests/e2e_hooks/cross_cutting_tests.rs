//! Cross-cutting tests: timeout, unexpected exit codes, matchers,
//! precedence, stdin JSON shape, and notification pipeline.

use agent_client_protocol::Agent;
use tokio::sync::broadcast;

use crate::helpers;
use std::sync::Arc;

/// Maximum time to wait for an async channel message in notification tests.
const CHANNEL_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(500);

/// A hook exiting with code 1 (or any code other than 0 or 2) should
/// be treated as Allow — the prompt proceeds normally.
#[tokio::test]
async fn unexpected_exit_code_allows() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 1, "some error");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("UserPromptSubmit", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = agent
        .prompt(helpers::make_prompt_request(session_id, "hello"))
        .await;

    // Exit code 1 → Allow → prompt should succeed
    assert!(
        result.is_ok(),
        "Unexpected exit code should allow prompt, got: {:?}",
        result.err()
    );
}

/// The all_event_kinds helper should return exactly 7 variants.
#[test]
fn all_event_kinds_is_exhaustive() {
    let kinds = helpers::all_event_kinds();
    assert_eq!(
        kinds.len(),
        7,
        "Expected 7 HookEventKind variants, got {}. \
         If you added a new variant, update all_event_kinds() and add e2e tests.",
        kinds.len()
    );
}

/// PreToolUse matcher "Bash" should fire for Bash but not Write tool calls.
#[tokio::test]
async fn matcher_filters_by_tool_name() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 0, "");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json =
        helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), Some("Bash"));
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    // Send Bash tool call — hook should fire
    helpers::send_named_tool_notification(&tx, "test-session", "Bash", "call-1").await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "Hook should fire for matching tool name 'Bash'"
    );

    // Clear capture file
    helpers::clear_stdin_capture(tmp.path(), "hook.sh");

    // Send Write tool call — hook should NOT fire
    helpers::send_named_tool_notification(&tx, "test-session", "Write", "call-2").await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_none(),
        "Hook should NOT fire for non-matching tool name 'Write'"
    );
}

/// Notification matcher "agent_message" should fire for AgentMessageChunk
/// but not for ToolCall notifications.
#[tokio::test]
async fn notification_matcher_filters_by_update_type() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 0, "");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json(
        "Notification",
        script.to_str().unwrap(),
        Some("agent_message"),
    );
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    // Send agent message — hook should fire
    helpers::send_agent_message_notification(&tx, "test-session").await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_some(),
        "Hook should fire for matching notification type 'agent_message'"
    );

    // Clear capture file
    helpers::clear_stdin_capture(tmp.path(), "hook.sh");

    // Send tool call (notification_update_name = "tool_call") — hook should NOT fire
    helpers::send_named_tool_notification(&tx, "test-session", "Bash", "call-1").await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(
        captured.is_none(),
        "Hook should NOT fire for non-matching notification type 'tool_call'"
    );
}

/// Command hook timeout should produce Block and reject the prompt.
#[tokio::test]
async fn timeout_blocks() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_slow_script(tmp.path(), "hook.sh", 5);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json =
        helpers::hook_config_json_with_timeout("UserPromptSubmit", script.to_str().unwrap(), 1);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = agent
        .prompt(helpers::make_prompt_request(session_id, "hello"))
        .await;

    assert!(result.is_err(), "Timeout hook should block the prompt");
    let err = result.unwrap_err();
    assert!(
        err.message.contains("timed out"),
        "Error should mention timeout, got: {}",
        err.message
    );
}

/// Multiple hooks precedence: Block wins over AllowWithContext.
#[tokio::test]
async fn precedence_block_wins_over_context() {
    let tmp = tempfile::TempDir::new().unwrap();

    // First hook: AllowWithContext
    let context_json = r#"{"additionalContext":"extra info"}"#;
    let context_script =
        helpers::write_json_output_script(tmp.path(), "context_hook.sh", context_json);

    // Second hook: Block
    let block_json = r#"{"decision":"block","reason":"policy violation"}"#;
    let block_script = helpers::write_json_output_script(tmp.path(), "block_hook.sh", block_json);

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_two_hooks_json(
        "UserPromptSubmit",
        context_script.to_str().unwrap(),
        block_script.to_str().unwrap(),
    );
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let result = agent
        .prompt(helpers::make_prompt_request(session_id, "hello"))
        .await;

    assert!(
        result.is_err(),
        "Block should take precedence over AllowWithContext"
    );
    let err = result.unwrap_err();
    assert!(
        err.message.contains("policy violation"),
        "Error should contain block reason, got: {}",
        err.message
    );
}

/// Stdin JSON for UserPromptSubmit should contain correct fields.
#[tokio::test]
async fn stdin_json_shape_user_prompt_submit() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 0, "");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("UserPromptSubmit", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let session_id = helpers::init_session(&agent).await;
    let _ = agent
        .prompt(helpers::make_prompt_request(session_id, "test prompt text"))
        .await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(captured.is_some(), "UserPromptSubmit hook should run");
    let json: serde_json::Value = serde_json::from_str(&captured.unwrap()).unwrap();
    assert_eq!(json["hook_event_name"], "UserPromptSubmit");
    assert!(
        json["session_id"].is_string(),
        "session_id should be present"
    );
    assert!(json["cwd"].is_string(), "cwd should be present");
    assert!(
        json["prompt"]
            .as_str()
            .unwrap()
            .contains("test prompt text"),
        "prompt should contain the submitted text"
    );
}

/// Stdin JSON for PreToolUse should contain tool_name.
#[tokio::test]
async fn stdin_json_shape_pre_tool_use() {
    let tmp = tempfile::TempDir::new().unwrap();
    let script = helpers::write_exit_script(tmp.path(), "hook.sh", 0, "");

    let playback = helpers::load_playback_agent("tool_call_session.json");
    let config_json = helpers::hook_config_json("PreToolUse", script.to_str().unwrap(), None);
    let agent = helpers::build_hookable_agent(Arc::new(playback), &config_json);

    let _session_id = helpers::init_session(&agent).await;

    let (tx, rx) = broadcast::channel(16);
    let (_forwarded_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(rx);

    helpers::send_named_tool_notification(&tx, "test-session", "Bash", "call-1").await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let captured = helpers::read_stdin_capture(tmp.path(), "hook.sh");
    assert!(captured.is_some(), "PreToolUse hook should run");
    let json: serde_json::Value = serde_json::from_str(&captured.unwrap()).unwrap();
    assert_eq!(json["hook_event_name"], "PreToolUse");
    assert!(
        json["session_id"].is_string(),
        "session_id should be present"
    );
    assert!(json["cwd"].is_string(), "cwd should be present");
    assert_eq!(json["tool_name"], "Bash", "tool_name should be present");
}

/// Notification pipeline delivers AllowWithContext via the context channel.
#[tokio::test]
async fn notification_pipeline_delivers_context() {
    let tmp = tempfile::TempDir::new().unwrap();
    let json_output = r#"{"additionalContext":"pipeline context delivery"}"#;
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
        "Notification AllowWithContext should deliver via context channel"
    );
    assert!(
        ctx.unwrap().unwrap().contains("pipeline context delivery"),
        "Context should contain the additionalContext value"
    );
}
