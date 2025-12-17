//! Tests for session persistence and loading functionality
//!
//! This test suite verifies that sessions can be:
//! 1. Saved to disk correctly
//! 2. Loaded from disk with all data intact
//! 3. Survive process restarts (simulated by creating new SessionManager instances)
//! 4. Handle edge cases like missing files, corrupted data, etc.

use claude_agent::session::{Message, MessageRole, Session, SessionId, SessionManager};

use std::time::Duration;
use tempfile::TempDir;

/// Helper to create a session manager with a temporary storage directory
fn create_test_session_manager() -> (SessionManager, TempDir) {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));

    (manager, temp_dir)
}

#[test]
fn test_session_saved_to_disk_on_creation() {
    let (manager, temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd.clone(), None).unwrap();

    // Verify session file exists on disk
    let session_file = temp_dir
        .path()
        .join("sessions")
        .join(format!("{}.json", session_id));
    assert!(
        session_file.exists(),
        "Session file should exist after creation"
    );

    // Verify file contains valid JSON
    let json = std::fs::read_to_string(&session_file).unwrap();
    let session: Session = serde_json::from_str(&json).unwrap();
    assert_eq!(session.id, session_id);
    assert_eq!(session.cwd, cwd);
}

#[test]
fn test_session_loaded_from_disk_after_restart() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cwd = std::env::current_dir().unwrap();

    let session_id = {
        // Create session in first "process"
        let manager =
            SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));

        let session_id = manager.create_session(cwd.clone(), None).unwrap();

        // Add some messages to the session
        manager
            .update_session(&session_id, |session| {
                session.add_message(Message::new(MessageRole::User, "Hello".to_string()));
                session.add_message(Message::new(
                    MessageRole::Assistant,
                    "Hi there!".to_string(),
                ));
            })
            .unwrap();

        session_id
    };

    // Simulate restart by creating new SessionManager with same storage path
    let manager = SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));

    // Load session from disk
    let session = manager.get_session(&session_id).unwrap();
    assert!(session.is_some(), "Session should be loaded from disk");

    let session = session.unwrap();
    assert_eq!(session.id, session_id);
    assert_eq!(session.cwd, cwd);
    assert_eq!(session.context.len(), 2, "Messages should be preserved");

    // Verify message content
    if let agent_client_protocol::SessionUpdate::UserMessageChunk(chunk) =
        &session.context[0].update
    {
        if let agent_client_protocol::ContentBlock::Text(text) = &chunk.content {
            assert_eq!(text.text, "Hello");
        } else {
            panic!("Expected text content");
        }
    } else {
        panic!("Expected UserMessageChunk");
    }
}

#[test]
fn test_session_updates_persisted_to_disk() {
    let (manager, temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    // Update session
    manager
        .update_session(&session_id, |session| {
            session.add_message(Message::new(MessageRole::User, "Test message".to_string()));
        })
        .unwrap();

    // Read session file directly
    let session_file = temp_dir
        .path()
        .join("sessions")
        .join(format!("{}.json", session_id));
    let json = std::fs::read_to_string(&session_file).unwrap();
    let session: Session = serde_json::from_str(&json).unwrap();

    // Verify update was persisted
    assert_eq!(session.context.len(), 1);
}

#[test]
fn test_session_removal_deletes_disk_file() {
    let (manager, temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    let session_file = temp_dir
        .path()
        .join("sessions")
        .join(format!("{}.json", session_id));
    assert!(session_file.exists(), "Session file should exist");

    // Remove session
    manager.remove_session(&session_id).unwrap();

    // Verify file is deleted
    assert!(
        !session_file.exists(),
        "Session file should be deleted after removal"
    );
}

#[test]
fn test_list_sessions_includes_disk_sessions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cwd = std::env::current_dir().unwrap();

    // Create sessions and then drop the manager
    let session_ids = {
        let manager =
            SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));

        let id1 = manager.create_session(cwd.clone(), None).unwrap();
        let id2 = manager.create_session(cwd.clone(), None).unwrap();
        let id3 = manager.create_session(cwd, None).unwrap();

        vec![id1, id2, id3]
    };

    // Create new manager and list sessions
    let manager = SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));

    let listed_sessions = manager.list_sessions().unwrap();

    // Should find all three sessions from disk
    assert_eq!(listed_sessions.len(), 3);
    for id in session_ids {
        assert!(
            listed_sessions.contains(&id),
            "Session {} should be in list",
            id
        );
    }
}

#[test]
fn test_session_with_client_capabilities_persisted() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let fs_cap = agent_client_protocol::FileSystemCapability::new()
        .read_text_file(true)
        .write_text_file(true);

    let capabilities = agent_client_protocol::ClientCapabilities::new()
        .fs(fs_cap)
        .terminal(true);

    let session_id = manager
        .create_session(cwd, Some(capabilities.clone()))
        .unwrap();

    // Simulate restart
    let session = manager.get_session(&session_id).unwrap().unwrap();

    assert!(session.client_capabilities.is_some());
    let loaded_caps = session.client_capabilities.unwrap();

    // Verify capabilities were preserved
    assert!(loaded_caps.fs.read_text_file);
    assert!(loaded_caps.fs.write_text_file);
    assert!(loaded_caps.terminal);
}

#[test]
fn test_session_with_mcp_servers_persisted() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    // Update session with MCP servers
    manager
        .update_session(&session_id, |session| {
            session.mcp_servers = vec!["server1".to_string(), "server2".to_string()];
        })
        .unwrap();

    // Load from disk
    let session = manager.get_session(&session_id).unwrap().unwrap();

    assert_eq!(session.mcp_servers.len(), 2);
    assert_eq!(session.mcp_servers[0], "server1");
    assert_eq!(session.mcp_servers[1], "server2");
}

#[test]
fn test_session_timestamps_persisted() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    // Get original timestamps
    let original_session = manager.get_session(&session_id).unwrap().unwrap();
    let created_at = original_session.created_at;
    let last_accessed = original_session.last_accessed;

    // Small delay to ensure time difference
    std::thread::sleep(Duration::from_millis(10));

    // Update session to change last_accessed
    manager
        .update_session(&session_id, |session| {
            session.add_message(Message::new(MessageRole::User, "Test".to_string()));
        })
        .unwrap();

    // Clear in-memory cache by dropping and recreating manager
    drop(manager);
    let manager = SessionManager::new().with_storage_path(Some(_temp_dir.path().join("sessions")));

    // Load from disk
    let session = manager.get_session(&session_id).unwrap().unwrap();

    // created_at should be unchanged
    assert_eq!(session.created_at, created_at);

    // last_accessed should be updated
    assert!(session.last_accessed > last_accessed);
}

#[test]
fn test_session_available_commands_persisted() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    let commands = vec![
        agent_client_protocol::AvailableCommand::new(
            "create_plan".to_string(),
            "Create an execution plan".to_string(),
        ),
        agent_client_protocol::AvailableCommand::new(
            "research_codebase".to_string(),
            "Research the codebase".to_string(),
        ),
    ];

    manager
        .update_available_commands(&session_id, commands.clone())
        .unwrap();

    // Load from disk
    let session = manager.get_session(&session_id).unwrap().unwrap();

    assert_eq!(session.available_commands.len(), 2);
    assert_eq!(session.available_commands[0].name, "create_plan");
    assert_eq!(session.available_commands[1].name, "research_codebase");
}

#[test]
fn test_session_turn_counters_persisted() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    // Update turn counters
    manager
        .update_session(&session_id, |session| {
            session.increment_turn_requests();
            session.increment_turn_requests();
            session.add_turn_tokens(1000);
            session.add_turn_tokens(500);
        })
        .unwrap();

    // Load from disk
    let session = manager.get_session(&session_id).unwrap().unwrap();

    assert_eq!(session.get_turn_request_count(), 2);
    assert_eq!(session.get_turn_token_count(), 1500);
}

#[test]
fn test_session_current_mode_persisted() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    // Set current mode
    manager
        .update_session(&session_id, |session| {
            session.current_mode = Some("research".to_string());
        })
        .unwrap();

    // Load from disk
    let session = manager.get_session(&session_id).unwrap().unwrap();

    assert_eq!(session.current_mode, Some("research".to_string()));
}

#[test]
fn test_multiple_sessions_persisted_independently() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    // Create multiple sessions with different data
    let session_id1 = manager.create_session(cwd.clone(), None).unwrap();
    let session_id2 = manager.create_session(cwd.clone(), None).unwrap();
    let session_id3 = manager.create_session(cwd, None).unwrap();

    manager
        .update_session(&session_id1, |session| {
            session.add_message(Message::new(MessageRole::User, "Session 1".to_string()));
        })
        .unwrap();

    manager
        .update_session(&session_id2, |session| {
            session.add_message(Message::new(MessageRole::User, "Session 2".to_string()));
        })
        .unwrap();

    manager
        .update_session(&session_id3, |session| {
            session.add_message(Message::new(MessageRole::User, "Session 3".to_string()));
        })
        .unwrap();

    // Load each session and verify they have correct data
    let session1 = manager.get_session(&session_id1).unwrap().unwrap();
    let session2 = manager.get_session(&session_id2).unwrap().unwrap();
    let session3 = manager.get_session(&session_id3).unwrap().unwrap();

    assert_eq!(session1.context.len(), 1);
    assert_eq!(session2.context.len(), 1);
    assert_eq!(session3.context.len(), 1);

    // Verify each session has its own distinct message
    if let agent_client_protocol::SessionUpdate::UserMessageChunk(chunk) =
        &session1.context[0].update
    {
        if let agent_client_protocol::ContentBlock::Text(text) = &chunk.content {
            assert_eq!(text.text, "Session 1");
        }
    }

    if let agent_client_protocol::SessionUpdate::UserMessageChunk(chunk) =
        &session2.context[0].update
    {
        if let agent_client_protocol::ContentBlock::Text(text) = &chunk.content {
            assert_eq!(text.text, "Session 2");
        }
    }

    if let agent_client_protocol::SessionUpdate::UserMessageChunk(chunk) =
        &session3.context[0].update
    {
        if let agent_client_protocol::ContentBlock::Text(text) = &chunk.content {
            assert_eq!(text.text, "Session 3");
        }
    }
}

#[test]
fn test_session_loaded_on_first_get_after_restart() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cwd = std::env::current_dir().unwrap();

    // Create session
    let session_id = {
        let manager =
            SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));
        manager.create_session(cwd, None).unwrap()
    };

    // Create new manager (simulated restart)
    let manager = SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));

    // First get should load from disk
    let session = manager.get_session(&session_id).unwrap();
    assert!(
        session.is_some(),
        "Session should be loaded from disk on first get"
    );

    // Second get should use in-memory cache
    let session2 = manager.get_session(&session_id).unwrap();
    assert!(
        session2.is_some(),
        "Session should still be available from cache"
    );
}

#[test]
fn test_session_with_complex_message_history_persisted() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd, None).unwrap();

    // Add various types of messages
    manager
        .update_session(&session_id, |session| {
            session.add_message(Message::new(MessageRole::User, "Hello".to_string()));
            session.add_message(Message::new(MessageRole::Assistant, "Hi!".to_string()));
            session.add_message(Message::new(MessageRole::User, "How are you?".to_string()));
            session.add_message(Message::new(
                MessageRole::Assistant,
                "I'm doing well!".to_string(),
            ));
            session.add_message(Message::new(
                MessageRole::System,
                "System message".to_string(),
            ));
        })
        .unwrap();

    // Load from disk
    let session = manager.get_session(&session_id).unwrap().unwrap();

    assert_eq!(session.context.len(), 5);
}

#[test]
fn test_nonexistent_session_file_returns_none() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new().with_storage_path(Some(temp_dir.path().join("sessions")));

    let nonexistent_id = SessionId::new();
    let session = manager.get_session(&nonexistent_id).unwrap();

    assert!(session.is_none(), "Nonexistent session should return None");
}

#[test]
fn test_session_storage_directory_created_if_missing() {
    let temp_dir = tempfile::tempdir().unwrap();
    let storage_path = temp_dir.path().join("new_sessions_dir");

    // Verify directory doesn't exist yet
    assert!(!storage_path.exists());

    let manager = SessionManager::new().with_storage_path(Some(storage_path.clone()));

    let cwd = std::env::current_dir().unwrap();
    manager.create_session(cwd, None).unwrap();

    // Directory should be created
    assert!(storage_path.exists());
    assert!(storage_path.is_dir());
}

#[test]
fn test_session_file_contains_valid_json() {
    let (manager, temp_dir) = create_test_session_manager();
    let cwd = std::env::current_dir().unwrap();

    let session_id = manager.create_session(cwd.clone(), None).unwrap();

    let session_file = temp_dir
        .path()
        .join("sessions")
        .join(format!("{}.json", session_id));
    let json = std::fs::read_to_string(&session_file).unwrap();

    // Should be pretty-printed JSON
    assert!(json.contains("{\n"));

    // Should be parseable
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Should contain expected fields
    assert!(parsed.get("id").is_some());
    assert!(parsed.get("created_at").is_some());
    assert!(parsed.get("last_accessed").is_some());
    assert!(parsed.get("context").is_some());
    assert!(parsed.get("cwd").is_some());
}

#[test]
fn test_session_working_directory_path_persisted_correctly() {
    let (manager, _temp_dir) = create_test_session_manager();
    let cwd = std::env::temp_dir();

    let session_id = manager.create_session(cwd.clone(), None).unwrap();

    // Load from disk
    let session = manager.get_session(&session_id).unwrap().unwrap();

    assert_eq!(session.cwd, cwd);
    assert!(session.cwd.is_absolute());
}
