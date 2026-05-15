//! Unit tests for long conversation data structures and utilities
//!
//! These tests verify the fundamental data structure behavior for long conversations
//! without requiring model initialization or actual inference.

use llama_agent::types::{
    ids::{SessionId, ToolCallId},
    Message, MessageRole,
};
use std::time::SystemTime;

/// Test that we can create and store many messages in memory
#[test]
fn test_message_vec_capacity() {
    let mut messages = Vec::new();

    // Create 1000 messages to simulate a long conversation
    for i in 0..1000 {
        let user_msg = Message {
            role: MessageRole::User,
            content: format!("User message {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        messages.push(user_msg);

        let assistant_msg = Message {
            role: MessageRole::Assistant,
            content: format!("Assistant response {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        messages.push(assistant_msg);
    }

    assert_eq!(messages.len(), 2000, "Should store 2000 messages");

    // Verify first and last messages
    assert_eq!(messages[0].content, "User message 0");
    assert_eq!(messages[1999].content, "Assistant response 999");
}

/// Test that message ordering is preserved in a long conversation
#[test]
fn test_message_ordering_preservation() {
    let mut messages = Vec::new();

    // Create messages with specific identifiable content
    for i in 0..100 {
        let msg = Message {
            role: MessageRole::User,
            content: format!("Message number {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        messages.push(msg);
    }

    // Verify all messages are in correct order
    for (idx, msg) in messages.iter().enumerate() {
        assert_eq!(
            msg.content,
            format!("Message number {}", idx),
            "Message {} should be in correct position",
            idx
        );
    }
}

/// Test that we can handle large message content
#[test]
fn test_large_message_content() {
    // Create a message with 100KB of content
    let large_content = "x".repeat(100_000);

    let msg = Message {
        role: MessageRole::User,
        content: large_content.clone(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };

    assert_eq!(msg.content.len(), 100_000);
    assert_eq!(msg.content, large_content);
}

/// Test that we can handle mixed message types in sequence
#[test]
fn test_mixed_message_types() {
    let mut messages = Vec::new();

    for i in 0..50 {
        // User message
        messages.push(Message {
            role: MessageRole::User,
            content: format!("User {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        // Assistant message
        messages.push(Message {
            role: MessageRole::Assistant,
            content: format!("Assistant {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        // Tool message every 5 turns
        if i % 5 == 0 {
            messages.push(Message {
                role: MessageRole::Tool,
                content: format!("Tool result {}", i),
                tool_call_id: Some(ToolCallId::new()),
                tool_name: Some("test_tool".to_string()),
                timestamp: SystemTime::now(),
            });
        }
    }

    // Count by role
    let user_count = messages
        .iter()
        .filter(|m| m.role == MessageRole::User)
        .count();
    let assistant_count = messages
        .iter()
        .filter(|m| m.role == MessageRole::Assistant)
        .count();
    let tool_count = messages
        .iter()
        .filter(|m| m.role == MessageRole::Tool)
        .count();

    assert_eq!(user_count, 50);
    assert_eq!(assistant_count, 50);
    assert_eq!(tool_count, 10); // Every 5th turn from 0..50
}

/// Test session ID generation and uniqueness
#[test]
fn test_session_id_uniqueness() {
    let mut session_ids = Vec::new();

    // Generate many session IDs
    for _ in 0..1000 {
        session_ids.push(SessionId::new());
    }

    // Check all are unique
    for i in 0..session_ids.len() {
        for j in (i + 1)..session_ids.len() {
            assert_ne!(
                session_ids[i], session_ids[j],
                "Session IDs should be unique"
            );
        }
    }
}

/// Test that message timestamps are sequential
#[test]
fn test_message_timestamps_sequential() {
    let mut messages = Vec::new();

    for i in 0..100 {
        let msg = Message {
            role: MessageRole::User,
            content: format!("Message {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        messages.push(msg);

        // Small delay to ensure timestamps are different
        std::thread::sleep(std::time::Duration::from_micros(100));
    }

    // Verify timestamps are in order
    for i in 1..messages.len() {
        assert!(
            messages[i].timestamp >= messages[i - 1].timestamp,
            "Timestamps should be sequential"
        );
    }
}

/// Test memory usage estimation for long conversations
#[test]
fn test_memory_usage_estimation() {
    let mut messages = Vec::new();

    // Create 1000 messages with average content
    for i in 0..1000 {
        let msg = Message {
            role: MessageRole::User,
            content: format!(
                "This is message {} with some content to simulate realistic usage",
                i
            ),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        messages.push(msg);
    }

    // Rough memory calculation:
    // Each message has ~60 bytes of content + overhead
    // 1000 messages * ~100 bytes (with overhead) = ~100KB
    // This should be well within memory limits

    assert_eq!(messages.len(), 1000);

    // Verify memory doesn't grow unexpectedly
    let total_content_size: usize = messages.iter().map(|m| m.content.len()).sum();

    // Should be around 60KB of actual content
    assert!(
        total_content_size > 50_000 && total_content_size < 100_000,
        "Content size should be reasonable: {}",
        total_content_size
    );
}

/// Test that we can efficiently filter messages by role
#[test]
fn test_message_filtering_performance() {
    let mut messages = Vec::new();

    // Create 10000 messages
    for i in 0..10000 {
        messages.push(Message {
            role: if i % 2 == 0 {
                MessageRole::User
            } else {
                MessageRole::Assistant
            },
            content: format!("Message {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });
    }

    // Filter by role - this should be fast
    let user_messages: Vec<_> = messages
        .iter()
        .filter(|m| m.role == MessageRole::User)
        .collect();

    assert_eq!(user_messages.len(), 5000);
}

/// Test conversation reconstruction from messages
#[test]
fn test_conversation_reconstruction() {
    let mut messages = Vec::new();

    // Create a conversation with specific pattern
    for i in 0..20 {
        messages.push(Message {
            role: MessageRole::User,
            content: format!("Question {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        messages.push(Message {
            role: MessageRole::Assistant,
            content: format!("Answer {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });
    }

    // Reconstruct conversation as text
    let mut conversation_text = String::new();
    for msg in &messages {
        match msg.role {
            MessageRole::User => conversation_text.push_str(&format!("User: {}\n", msg.content)),
            MessageRole::Assistant => {
                conversation_text.push_str(&format!("Assistant: {}\n", msg.content))
            }
            MessageRole::Tool => conversation_text.push_str(&format!("Tool: {}\n", msg.content)),
            MessageRole::System => {
                conversation_text.push_str(&format!("System: {}\n", msg.content))
            }
        }
    }

    // Verify reconstruction includes all messages
    assert!(conversation_text.contains("Question 0"));
    assert!(conversation_text.contains("Answer 19"));
    assert!(!conversation_text.is_empty());
}

/// Test that tool call IDs are unique
#[test]
fn test_tool_call_id_uniqueness() {
    let mut tool_call_ids = Vec::new();

    // Generate many tool call IDs
    for _ in 0..1000 {
        tool_call_ids.push(ToolCallId::new());
    }

    // Check all are unique
    for i in 0..tool_call_ids.len() {
        for j in (i + 1)..tool_call_ids.len() {
            assert_ne!(
                tool_call_ids[i], tool_call_ids[j],
                "Tool call IDs should be unique"
            );
        }
    }
}
