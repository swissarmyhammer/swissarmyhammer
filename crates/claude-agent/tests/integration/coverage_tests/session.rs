//! Coverage tests: Conversation types, sessions, session manager and session ids.

use agent_client_protocol::schema::{ContentBlock, ContentChunk, SessionUpdate, TextContent};
use serde_json::json;
use std::path::PathBuf;
use std::time::SystemTime;

// ============================================================================
// Conversation manager type tests
// ============================================================================

#[test]
fn test_token_usage_default() {
    let usage = claude_agent::conversation_manager::TokenUsage::default();
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.total(), 0);
}

#[test]
fn test_token_usage_estimate() {
    let usage =
        claude_agent::conversation_manager::TokenUsage::estimate_from_text("hello world", "hi");
    // 11 chars / 4 = 2, 2 chars / 4 = 0
    assert_eq!(usage.input_tokens, 2);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.total(), 2);
}

#[test]
fn test_token_usage_estimate_longer() {
    let input = "a".repeat(100);
    let output = "b".repeat(200);
    let usage = claude_agent::conversation_manager::TokenUsage::estimate_from_text(&input, &output);
    assert_eq!(usage.input_tokens, 25);
    assert_eq!(usage.output_tokens, 50);
    assert_eq!(usage.total(), 75);
}

#[test]
fn test_tool_execution_status_eq() {
    use claude_agent::conversation_manager::ToolExecutionStatus;
    assert_eq!(ToolExecutionStatus::Success, ToolExecutionStatus::Success);
    assert_eq!(ToolExecutionStatus::Error, ToolExecutionStatus::Error);
    assert_ne!(ToolExecutionStatus::Success, ToolExecutionStatus::Error);
}

#[test]
fn test_tool_call_request_serialization() {
    use claude_agent::conversation_manager::ToolCallRequest;
    let req = ToolCallRequest {
        id: "call_123".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({"path": "/tmp/test.txt"}),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("call_123"));
    assert!(json.contains("fs_read"));
    let deserialized: ToolCallRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "call_123");
}

// ============================================================================
// Session tests
// ============================================================================

/// Length of a ULID in text form.
const ULID_LENGTH: usize = 26;

/// Build a text chunk of the kind a message carries.
fn text_chunk(text: &str) -> ContentChunk {
    ContentChunk::new(ContentBlock::Text(TextContent::new(text.to_string())))
}

/// Check that a message carries the expected update and was stamped since `before`.
fn assert_recorded(
    msg: &claude_agent::session::Message,
    expected: &SessionUpdate,
    before: SystemTime,
) {
    assert_eq!(
        serde_json::to_value(&msg.update).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert!(msg.timestamp >= before && msg.timestamp <= SystemTime::now());
}

#[test]
fn test_session_id_new() {
    let id = claude_agent::session::SessionId::new();
    let s = id.to_string();
    assert_eq!(s.len(), ULID_LENGTH);
}

#[test]
fn test_session_id_parse_valid() {
    let id = claude_agent::session::SessionId::new();
    let s = id.to_string();
    let parsed = claude_agent::session::SessionId::parse(&s).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn test_session_id_parse_empty() {
    let result = claude_agent::session::SessionId::parse("");
    assert!(result.is_err());
}

#[test]
fn test_session_id_parse_invalid() {
    let result = claude_agent::session::SessionId::parse("not-a-ulid");
    assert!(result.is_err());
}

#[test]
fn test_session_id_default() {
    let id = claude_agent::session::SessionId::default();
    let parsed = claude_agent::session::SessionId::parse(&id.to_string()).unwrap();
    assert_eq!(parsed, id);
}

#[test]
fn test_session_id_from_str() {
    let id = claude_agent::session::SessionId::new();
    let s = id.to_string();
    let parsed: claude_agent::session::SessionId = s.parse().unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn test_session_id_serialization() {
    let id = claude_agent::session::SessionId::new();
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: claude_agent::session::SessionId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, deserialized);
}

#[test]
fn test_session_id_to_uuid_string() {
    let id = claude_agent::session::SessionId::new();
    let uuid = id.to_uuid_string();
    // UUID format: 8-4-4-4-12
    assert_eq!(uuid.len(), 36);
    assert_eq!(uuid.chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn test_session_id_to_ulid_string() {
    let id = claude_agent::session::SessionId::new();
    let ulid = id.to_ulid_string();
    assert_eq!(ulid.len(), ULID_LENGTH);
    assert_eq!(ulid, id.to_string());
}

#[test]
fn test_session_id_as_ulid() {
    let id = claude_agent::session::SessionId::new();
    let ulid = id.as_ulid();
    assert_eq!(ulid.to_string(), id.to_string());
}

#[test]
fn test_session_id_from_ulid() {
    let ulid = ulid::Ulid::new();
    let id: claude_agent::session::SessionId = ulid.into();
    assert_eq!(id.as_ulid(), ulid);
}

#[test]
fn test_session_new() {
    let id = claude_agent::session::SessionId::new();
    let session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"))
        .expect("test working directory must be absolute");
    assert_eq!(session.id, id);
    assert_eq!(session.cwd, PathBuf::from("/tmp"));
    assert!(session.context.is_empty());
    assert_eq!(session.turn_request_count, 0);
    assert_eq!(session.turn_token_count, 0);
    assert!(session.current_mode.is_none());
}

#[test]
fn test_session_add_message() {
    let id = claude_agent::session::SessionId::new();
    let mut session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"))
        .expect("test working directory must be absolute");
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::User,
        "hello".to_string(),
    );
    session.add_message(msg);
    assert_eq!(session.context.len(), 1);
}

#[test]
fn test_session_turn_counters() {
    let id = claude_agent::session::SessionId::new();
    let mut session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"))
        .expect("test working directory must be absolute");

    assert_eq!(session.turn_request_count(), 0);
    assert_eq!(session.turn_token_count(), 0);

    let count = session.increment_turn_requests();
    assert_eq!(count, 1);
    assert_eq!(session.turn_request_count(), 1);

    let count = session.increment_turn_requests();
    assert_eq!(count, 2);

    let tokens = session.add_turn_tokens(100);
    assert_eq!(tokens, 100);
    assert_eq!(session.turn_token_count(), 100);

    let tokens = session.add_turn_tokens(50);
    assert_eq!(tokens, 150);

    session.reset_turn_counters();
    assert_eq!(session.turn_request_count(), 0);
    assert_eq!(session.turn_token_count(), 0);
}

#[test]
fn test_session_available_commands() {
    let id = claude_agent::session::SessionId::new();
    let mut session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"))
        .expect("test working directory must be absolute");

    let cmd1 = agent_client_protocol::schema::AvailableCommand::new("test", "A test command");

    // Initially empty
    assert!(!session.has_available_commands_changed(&[]));
    assert!(session.has_available_commands_changed(std::slice::from_ref(&cmd1)));

    session.update_available_commands(vec![cmd1.clone()]);
    assert!(!session.has_available_commands_changed(&[cmd1]));
}

#[test]
fn test_message_from_update() {
    let before = SystemTime::now();
    let update = SessionUpdate::AgentMessageChunk(text_chunk("hello"));
    let msg = claude_agent::session::Message::from_update(update.clone());
    assert_recorded(&msg, &update, before);
}

#[test]
fn test_message_new_user() {
    let before = SystemTime::now();
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::User,
        "hello".to_string(),
    );
    let expected = SessionUpdate::UserMessageChunk(text_chunk("hello"));
    assert_recorded(&msg, &expected, before);
}

#[test]
fn test_message_new_assistant() {
    let before = SystemTime::now();
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::Assistant,
        "response".to_string(),
    );
    let expected = SessionUpdate::AgentMessageChunk(text_chunk("response"));
    assert_recorded(&msg, &expected, before);
}

#[test]
fn test_message_new_system() {
    let before = SystemTime::now();
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::System,
        "system msg".to_string(),
    );
    let expected = SessionUpdate::AgentMessageChunk(text_chunk("[System] system msg"));
    assert_recorded(&msg, &expected, before);
}

// ============================================================================
// SessionManager tests
// ============================================================================

#[test]
fn test_session_manager_new_isolated() {
    let mgr = claude_agent::session::SessionManager::new();
    assert!(mgr.list_sessions().unwrap().is_empty());
}

#[test]
fn test_session_manager_create_and_get_session() {
    let mgr = claude_agent::session::SessionManager::new();
    let id = mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    let session = mgr.get_session(&id).unwrap();
    assert!(session.is_some());
    let session = session.unwrap();
    assert_eq!(session.id, id);
}

#[test]
fn test_session_manager_list_sessions() {
    let mgr = claude_agent::session::SessionManager::new();
    mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    let sessions = mgr.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_session_manager_remove_session() {
    let mgr = claude_agent::session::SessionManager::new();
    let id = mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    assert!(mgr.get_session(&id).unwrap().is_some());
    let _ = mgr.remove_session(&id);
    assert!(mgr.get_session(&id).unwrap().is_none());
}

// ============================================================================
// SessionIdError tests
// ============================================================================

#[test]
fn test_session_id_error_empty_display() {
    let err = claude_agent::session::SessionId::parse("").unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_session_id_error_invalid_display() {
    let err = claude_agent::session::SessionId::parse("INVALID!!!").unwrap_err();
    assert!(err.to_string().contains("invalid ULID"));
}
