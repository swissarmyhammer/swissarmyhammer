//! AVP schema validation tests.
//!
//! These tests verify that the JSON produced by `HookEvent::to_command_input_full()`
//! deserializes cleanly through AVP's strongly-typed input structs. This catches
//! field-name mismatches (e.g. `tool_response` vs `tool_result`) and missing
//! required fields (e.g. `transcript_path`, `tool_input`) at compile/test time
//! rather than at runtime in production.

use agent_client_protocol::{ContentBlock, StopReason, TextContent};
use agent_client_protocol_extras::{HookEvent, SessionSource};
use avp_common::HookInput;
use std::path::PathBuf;

use crate::helpers::avp_test_context;

/// SessionStart JSON deserializes to `HookInput::SessionStart`.
#[test]
fn avp_schema_session_start() {
    let event = HookEvent::SessionStart {
        session_id: "sess-avp-1".into(),
        source: SessionSource::Startup,
        cwd: PathBuf::from("/home/user/project"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput =
        serde_json::from_value(json).expect("SessionStart JSON should deserialize as HookInput");

    assert!(
        matches!(input, HookInput::SessionStart(_)),
        "Expected HookInput::SessionStart, got {:?}",
        input.hook_type()
    );
}

/// UserPromptSubmit JSON deserializes, `prompt` field correct.
#[test]
fn avp_schema_user_prompt_submit() {
    let event = HookEvent::UserPromptSubmit {
        session_id: "sess-avp-2".into(),
        prompt: vec![ContentBlock::Text(TextContent::new("Write a function"))],
        cwd: PathBuf::from("/home/user/project"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput = serde_json::from_value(json)
        .expect("UserPromptSubmit JSON should deserialize as HookInput");

    match input {
        HookInput::UserPromptSubmit(inner) => {
            assert_eq!(inner.prompt, "Write a function");
        }
        other => panic!(
            "Expected HookInput::UserPromptSubmit, got {:?}",
            other.hook_type()
        ),
    }
}

/// PreToolUse JSON deserializes, `tool_name` and `tool_input` correct.
#[test]
fn avp_schema_pre_tool_use() {
    let event = HookEvent::PreToolUse {
        session_id: "sess-avp-3".into(),
        tool_name: "Bash".into(),
        tool_input: Some(serde_json::json!({"command": "npm test"})),
        tool_use_id: Some("toolu_pre_1".into()),
        cwd: PathBuf::from("/home/user/project"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput =
        serde_json::from_value(json).expect("PreToolUse JSON should deserialize as HookInput");

    match input {
        HookInput::PreToolUse(inner) => {
            assert_eq!(inner.tool_name, "Bash");
            assert_eq!(inner.tool_input["command"], "npm test");
        }
        other => panic!(
            "Expected HookInput::PreToolUse, got {:?}",
            other.hook_type()
        ),
    }
}

/// PreToolUse with `None` tool_input still deserializes (defaults to `{}`).
#[test]
fn avp_schema_pre_tool_use_no_input() {
    let event = HookEvent::PreToolUse {
        session_id: "sess-avp-3b".into(),
        tool_name: "Read".into(),
        tool_input: None,
        tool_use_id: None,
        cwd: PathBuf::from("/tmp"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput = serde_json::from_value(json)
        .expect("PreToolUse with None tool_input should deserialize as HookInput");

    match input {
        HookInput::PreToolUse(inner) => {
            assert_eq!(inner.tool_name, "Read");
            assert!(inner.tool_input.is_object());
        }
        other => panic!(
            "Expected HookInput::PreToolUse, got {:?}",
            other.hook_type()
        ),
    }
}

/// PostToolUse JSON deserializes, `tool_result` populated from our `tool_response` field.
#[test]
fn avp_schema_post_tool_use() {
    let event = HookEvent::PostToolUse {
        session_id: "sess-avp-4".into(),
        tool_name: "Write".into(),
        tool_input: Some(serde_json::json!({"file_path": "/tmp/out.txt"})),
        tool_response: Some(serde_json::json!({"success": true})),
        tool_use_id: Some("toolu_post_1".into()),
        cwd: PathBuf::from("/home/user/project"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput =
        serde_json::from_value(json).expect("PostToolUse JSON should deserialize as HookInput");

    match input {
        HookInput::PostToolUse(inner) => {
            assert_eq!(inner.tool_name, "Write");
            // Our `tool_response` field should be deserialized into AVP's `tool_result`
            let result = inner
                .tool_result
                .expect("tool_result should be populated from tool_response");
            assert_eq!(result["success"], true);
        }
        other => panic!(
            "Expected HookInput::PostToolUse, got {:?}",
            other.hook_type()
        ),
    }
}

/// PostToolUseFailure JSON deserializes, `error` field correct.
#[test]
fn avp_schema_post_tool_use_failure() {
    let event = HookEvent::PostToolUseFailure {
        session_id: "sess-avp-5".into(),
        tool_name: "Bash".into(),
        tool_input: Some(serde_json::json!({"command": "false"})),
        error: Some(serde_json::json!("exit code 1")),
        tool_use_id: Some("toolu_fail_1".into()),
        cwd: PathBuf::from("/home/user/project"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput = serde_json::from_value(json)
        .expect("PostToolUseFailure JSON should deserialize as HookInput");

    match input {
        HookInput::PostToolUseFailure(inner) => {
            assert_eq!(inner.tool_name, "Bash");
            let error = inner.error.expect("error should be populated");
            assert_eq!(error, "exit code 1");
        }
        other => panic!(
            "Expected HookInput::PostToolUseFailure, got {:?}",
            other.hook_type()
        ),
    }
}

/// Stop JSON deserializes, `stop_hook_active` correct.
#[test]
fn avp_schema_stop() {
    let event = HookEvent::Stop {
        session_id: "sess-avp-6".into(),
        stop_reason: StopReason::EndTurn,
        stop_hook_active: false,
        cwd: PathBuf::from("/home/user/project"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput =
        serde_json::from_value(json).expect("Stop JSON should deserialize as HookInput");

    match input {
        HookInput::Stop(inner) => {
            assert!(!inner.stop_hook_active);
        }
        other => panic!("Expected HookInput::Stop, got {:?}", other.hook_type()),
    }
}

/// Notification JSON deserializes, `notification_type` correct.
#[test]
fn avp_schema_notification() {
    use agent_client_protocol::{ContentChunk, SessionId, SessionNotification, SessionUpdate};

    let content = ContentChunk::new(ContentBlock::Text(TextContent::new("hello")));
    let notification = SessionNotification::new(
        SessionId::from("sess-avp-7"),
        SessionUpdate::AgentMessageChunk(content),
    );

    let event = HookEvent::Notification {
        notification: Box::new(notification),
        cwd: PathBuf::from("/home/user/project"),
    };

    let json = event.to_command_input_full(&avp_test_context());
    let input: HookInput =
        serde_json::from_value(json).expect("Notification JSON should deserialize as HookInput");

    match input {
        HookInput::Notification(inner) => {
            assert_eq!(inner.notification_type.as_deref(), Some("agent_message"));
        }
        other => panic!(
            "Expected HookInput::Notification, got {:?}",
            other.hook_type()
        ),
    }
}
