//! Tests for token usage calculation and token counting functionality

use crate::tests::test_utils::*;
use crate::types::*;
use std::collections::HashMap;
use std::time::SystemTime;

#[cfg(test)]
mod token_usage {
    use super::*;

    #[test]
    fn test_token_usage_creation() {
        let mut by_role = HashMap::new();
        by_role.insert(MessageRole::User, 50);
        by_role.insert(MessageRole::Assistant, 50);

        let usage = TokenUsage {
            total: 100,
            by_role,
            by_message: vec![10, 20, 30, 40],
        };

        assert_eq!(usage.total, 100);
        assert_eq!(usage.by_message.len(), 4);
        assert_eq!(usage.by_role.get(&MessageRole::User), Some(&50));
        assert_eq!(usage.by_role.get(&MessageRole::Assistant), Some(&50));
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();

        assert_eq!(usage.total, 0);
        assert!(usage.by_role.is_empty());
        assert!(usage.by_message.is_empty());
    }

    #[test]
    fn test_token_usage_new() {
        let usage = TokenUsage::new();

        assert_eq!(usage.total, 0);
        assert!(usage.by_role.is_empty());
        assert!(usage.by_message.is_empty());
    }

    #[test]
    fn test_simple_token_counter_creation() {
        let counter = SimpleTokenCounter::new();
        assert!(std::mem::size_of_val(&counter) == 0); // Zero-sized type

        let counter_default = SimpleTokenCounter;
        assert!(std::mem::size_of_val(&counter_default) == 0);
    }

    #[test]
    fn test_simple_token_counter_basic_text() {
        let counter = SimpleTokenCounter;

        // Test basic text counting
        let text = "Hello world this is a test";
        let count = counter.count_tokens(text);

        // SimpleTokenCounter uses word-based approximation
        // "Hello world this is a test" = 6 words, roughly 6-8 tokens
        assert!(
            (5..=10).contains(&count),
            "Expected 5-10 tokens, got {}",
            count
        );
    }

    #[test]
    fn test_simple_token_counter_empty_text() {
        let counter = SimpleTokenCounter;
        let count = counter.count_tokens("");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_simple_token_counter_whitespace_text() {
        let counter = SimpleTokenCounter;
        let count = counter.count_tokens("   ");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_simple_token_counter_single_word() {
        let counter = SimpleTokenCounter;
        let count = counter.count_tokens("hello");
        assert_eq!(count, 1);
    }

    #[test]
    fn test_simple_token_counter_punctuation() {
        let counter = SimpleTokenCounter;
        let text = "Hello, world! How are you?";
        let count = counter.count_tokens(text);

        // Should handle punctuation reasonably
        assert!(
            (4..=8).contains(&count),
            "Expected 4-8 tokens, got {}",
            count
        );
    }

    #[test]
    fn test_message_token_counting() {
        let counter = SimpleTokenCounter;
        let message = Message {
            role: MessageRole::User,
            content: "Hello world".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };

        let count = counter.count_message_tokens(&message);

        // Should include role overhead + content tokens
        // "Hello world" = 2 words + role overhead
        assert!(
            (8..=12).contains(&count),
            "Expected 8-12 tokens, got {}",
            count
        );
    }

    #[test]
    fn test_message_token_counting_with_tool_name() {
        let counter = SimpleTokenCounter;
        let message = Message {
            role: MessageRole::Tool,
            content: "Tool result data".to_string(),
            tool_call_id: Some(ToolCallId::new()),
            tool_name: Some("test_tool".to_string()),
            timestamp: SystemTime::now(),
        };

        let count = counter.count_message_tokens(&message);

        // Should include role + tool name + content
        assert!(
            (15..=20).contains(&count),
            "Expected 15-20 tokens, got {}",
            count
        );
    }

    #[test]
    fn test_message_token_counting_assistant_role() {
        let counter = SimpleTokenCounter;
        let message = Message {
            role: MessageRole::Assistant,
            content: "I can help you with that task".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };

        let count = counter.count_message_tokens(&message);

        // "I can help you with that task" = 8 words + role overhead
        assert!(
            (18..=25).contains(&count),
            "Expected 18-25 tokens, got {}",
            count
        );
    }

    #[test]
    fn test_session_token_usage() {
        let counter = SimpleTokenCounter;
        let session = create_test_session_with_messages(5);

        let usage = counter.count_session_tokens(&session);

        assert!(usage.total > 0, "Total tokens should be greater than 0");
        assert_eq!(usage.by_message.len(), session.messages.len());

        // Should have tokens for both User and Assistant roles
        assert!(usage.by_role.contains_key(&MessageRole::User));
        assert!(usage.by_role.contains_key(&MessageRole::Assistant));

        // Verify total matches sum of by_role
        let role_total: usize = usage.by_role.values().sum();
        assert_eq!(usage.total, role_total);

        // Verify total matches sum of by_message
        let message_total: usize = usage.by_message.iter().sum();
        assert_eq!(usage.total, message_total);
    }

    #[test]
    fn test_session_token_usage_empty_session() {
        let counter = SimpleTokenCounter;
        let session = create_test_session_with_messages(0);

        let usage = counter.count_session_tokens(&session);

        assert_eq!(usage.total, 0);
        assert!(usage.by_role.is_empty());
        assert!(usage.by_message.is_empty());
    }

    #[test]
    fn test_session_token_usage_various_roles() {
        let counter = SimpleTokenCounter;
        let mut session = create_test_session_with_messages(0);

        // Add messages with different roles
        session.messages.push(Message {
            role: MessageRole::System,
            content: "System initialization message".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        session.messages.push(Message {
            role: MessageRole::User,
            content: "User question".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        session.messages.push(Message {
            role: MessageRole::Assistant,
            content: "Assistant response".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        });

        session.messages.push(Message {
            role: MessageRole::Tool,
            content: "Tool execution result".to_string(),
            tool_call_id: Some(ToolCallId::new()),
            tool_name: Some("test_tool".to_string()),
            timestamp: SystemTime::now(),
        });

        let usage = counter.count_session_tokens(&session);

        assert!(usage.total > 0);
        assert_eq!(usage.by_message.len(), 4);

        // Should have all four role types
        assert!(usage.by_role.contains_key(&MessageRole::System));
        assert!(usage.by_role.contains_key(&MessageRole::User));
        assert!(usage.by_role.contains_key(&MessageRole::Assistant));
        assert!(usage.by_role.contains_key(&MessageRole::Tool));
    }

    #[test]
    fn test_session_token_usage_large_content() {
        let counter = SimpleTokenCounter;
        let session = create_large_content_session(10, 100); // 10 messages, 100 words each

        let usage = counter.count_session_tokens(&session);

        // Should have substantial token count
        assert!(
            usage.total > 500,
            "Expected > 500 tokens for large content, got {}",
            usage.total
        );
        assert_eq!(usage.by_message.len(), 10);

        // Each message should have roughly similar token counts (since they have similar content)
        let min_tokens = usage.by_message.iter().min().unwrap();
        let max_tokens = usage.by_message.iter().max().unwrap();

        // Variance should be reasonable (within 50% of each other)
        assert!(
            *max_tokens <= min_tokens * 2,
            "Token counts vary too much: min={}, max={}",
            min_tokens,
            max_tokens
        );
    }

    #[test]
    fn test_token_counter_consistency() {
        let counter = SimpleTokenCounter;
        let text = "This is a consistent test message";

        // Multiple counts of the same text should be identical
        let count1 = counter.count_tokens(text);
        let count2 = counter.count_tokens(text);
        let count3 = counter.count_tokens(text);

        assert_eq!(count1, count2);
        assert_eq!(count2, count3);
    }

    #[test]
    fn test_message_role_string_representation() {
        assert_eq!(MessageRole::System.as_str(), "system");
        assert_eq!(MessageRole::User.as_str(), "user");
        assert_eq!(MessageRole::Assistant.as_str(), "assistant");
        assert_eq!(MessageRole::Tool.as_str(), "tool");
    }

    #[test]
    fn test_token_usage_with_real_session_data() {
        let counter = SimpleTokenCounter;
        let session = create_session_with_tool_calls();

        let usage = counter.count_session_tokens(&session);

        assert!(usage.total > 0);
        assert_eq!(usage.by_message.len(), 3); // User + Assistant + Tool messages

        // Should have tokens for User, Assistant, and Tool roles
        assert!(usage.by_role.contains_key(&MessageRole::User));
        assert!(usage.by_role.contains_key(&MessageRole::Assistant));
        assert!(usage.by_role.contains_key(&MessageRole::Tool));

        // Tool messages should include overhead for tool names
        let tool_tokens = usage.by_role.get(&MessageRole::Tool).unwrap();
        assert!(
            tool_tokens > &5,
            "Tool messages should have reasonable token count including overhead"
        );
    }

    #[test]
    fn test_create_test_token_usage_helper() {
        let usage = create_test_token_usage(300, vec![50, 100, 150]);

        assert_eq!(usage.total, 300);
        assert_eq!(usage.by_message, vec![50, 100, 150]);
        assert_eq!(usage.by_role.get(&MessageRole::User), Some(&100));
        assert_eq!(usage.by_role.get(&MessageRole::Assistant), Some(&200));
    }
}
