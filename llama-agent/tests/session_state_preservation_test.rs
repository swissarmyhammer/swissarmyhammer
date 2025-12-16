//! Test for session state preservation
//!
//! This test verifies that all session state (messages, tools, prompts, metadata)
//! is correctly preserved when a session is saved to storage and then loaded back.

use llama_agent::types::{Message, MessageRole, SessionConfig, ToolCallId};
use llama_agent::SessionManager;
use std::time::SystemTime;
use tempfile::TempDir;

#[tokio::test]
async fn test_session_state_preservation() {
    // Helper to create messages with proper fields
    fn create_message(
        role: MessageRole,
        content: &str,
        tool_call_id: Option<ToolCallId>,
        tool_name: Option<String>,
    ) -> Message {
        Message {
            role,
            content: content.to_string(),
            tool_call_id,
            tool_name,
            timestamp: SystemTime::now(),
        }
    }

    // Create a session manager with persistence enabled
    let temp_dir = TempDir::new().unwrap();
    let config = SessionConfig {
        max_sessions: 10,
        auto_compaction: None,
        model_context_size: 4096,
        persistence_enabled: true,
        session_storage_dir: Some(temp_dir.path().to_path_buf()),
        session_ttl_hours: 24,
        auto_save_threshold: 5,
        max_kv_cache_files: 16,
        kv_cache_dir: None,
    };

    let manager = SessionManager::new(config.clone());

    // Create a session with comprehensive state
    let session = manager.create_session().await.unwrap();
    let session_id = session.id;

    // Add multiple messages with different roles
    let messages = [
        create_message(
            MessageRole::System,
            "You are a helpful assistant.",
            None,
            None,
        ),
        create_message(MessageRole::User, "What is the weather today?", None, None),
        create_message(
            MessageRole::Assistant,
            "Let me check the weather for you.",
            None,
            None,
        ),
        create_message(
            MessageRole::Tool,
            r#"{"temperature": 72, "condition": "sunny"}"#,
            Some(ToolCallId::new()),
            Some("get_weather".to_string()),
        ),
        create_message(
            MessageRole::Assistant,
            "The weather is sunny with a temperature of 72°F.",
            None,
            None,
        ),
    ];

    for message in messages.iter() {
        manager
            .add_message(&session_id, message.clone())
            .await
            .unwrap();
    }

    // Get the original session to verify original state
    let original_session = manager.get_session(&session_id).await.unwrap().unwrap();

    // Save the session
    manager.save_session(&session_id).await.unwrap();

    // Create a new manager with the same storage directory to simulate a restart
    let manager2 = SessionManager::new(config);

    // Restore sessions from storage
    let restored_count = manager2.restore_sessions().await.unwrap();
    assert_eq!(restored_count, 1, "Should restore exactly one session");

    // Get the restored session
    let restored_session = manager2
        .get_session(&session_id)
        .await
        .unwrap()
        .expect("Session should exist after restoration");

    // Verify all session state is preserved

    // 1. Session ID
    assert_eq!(
        restored_session.id, session_id,
        "Session ID should be preserved"
    );

    // 2. Messages - count and content
    assert_eq!(
        restored_session.messages.len(),
        messages.len(),
        "All messages should be preserved"
    );

    for (i, (original, restored)) in messages
        .iter()
        .zip(restored_session.messages.iter())
        .enumerate()
    {
        assert_eq!(
            restored.role, original.role,
            "Message {} role should be preserved",
            i
        );
        assert_eq!(
            restored.content, original.content,
            "Message {} content should be preserved",
            i
        );
        assert_eq!(
            restored.tool_call_id, original.tool_call_id,
            "Message {} tool_call_id should be preserved",
            i
        );
        assert_eq!(
            restored.tool_name, original.tool_name,
            "Message {} tool_name should be preserved",
            i
        );
    }

    // 3. Tools (verify they match original session)
    assert_eq!(
        restored_session.available_tools.len(),
        original_session.available_tools.len(),
        "Tool count should be preserved"
    );

    for (i, (original, restored)) in original_session
        .available_tools
        .iter()
        .zip(restored_session.available_tools.iter())
        .enumerate()
    {
        assert_eq!(
            restored.name, original.name,
            "Tool {} name should be preserved",
            i
        );
        assert_eq!(
            restored.description, original.description,
            "Tool {} description should be preserved",
            i
        );
        assert_eq!(
            restored.parameters, original.parameters,
            "Tool {} parameters should be preserved",
            i
        );
    }

    // 4. Metadata timestamps
    assert!(
        restored_session.created_at <= SystemTime::now(),
        "Created timestamp should be valid"
    );
    assert!(
        restored_session.updated_at <= SystemTime::now(),
        "Updated timestamp should be valid"
    );

    // 5. Collections should be preserved
    assert_eq!(
        restored_session.mcp_servers.len(),
        original_session.mcp_servers.len(),
        "MCP servers list should be preserved"
    );
    assert_eq!(
        restored_session.available_prompts.len(),
        original_session.available_prompts.len(),
        "Prompts list should be preserved"
    );
    assert_eq!(
        restored_session.compaction_history.len(),
        original_session.compaction_history.len(),
        "Compaction history should be preserved"
    );

    // 6. Optional fields
    assert_eq!(
        restored_session.transcript_path, original_session.transcript_path,
        "transcript_path should be preserved"
    );
    assert_eq!(
        restored_session.template_token_count, original_session.template_token_count,
        "template_token_count should be preserved"
    );

    // 7. ACP-specific fields (when feature is enabled)

    {
        // Verify current_mode is preserved (always available now)
        assert_eq!(
            restored_session.current_mode, original_session.current_mode,
            "current_mode should be preserved"
        );
    }
}
