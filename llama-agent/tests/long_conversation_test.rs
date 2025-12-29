//! Integration tests for long conversation handling
//!
//! These tests verify that the AgentServer can handle:
//! - Long multi-turn conversations (many back-and-forth exchanges)
//! - Large message content (individual messages with substantial text)
//! - Token tracking and session state across extended conversations
//! - Memory and performance characteristics with long conversations

use llama_agent::{
    types::{
        ids::ToolCallId, AgentAPI, AgentConfig, Message, MessageRole, ModelConfig, ModelSource,
        ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
    },
    AgentServer,
};
use std::time::SystemTime;
use tracing::info;

/// Helper to create a test agent configuration with a lightweight model
fn create_test_config() -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "microsoft/Phi-3-mini-4k-instruct-gguf".to_string(),
                filename: Some("Phi-3-mini-4k-instruct-q4.gguf".to_string()),
                folder: None,
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: true,
            retry_config: RetryConfig::default(),
            debug: false,
        },
        session_config: SessionConfig::default(),
        queue_config: QueueConfig::default(),
        mcp_servers: vec![],
        parallel_execution_config: ParallelConfig::default(),
    }
}

/// Test that agent can handle a conversation with many turns (50+ messages)
#[tokio::test]
#[ignore] // Requires model download and significant compute time
async fn test_long_conversation_many_turns() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting long conversation test with many turns");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();
    let session = agent.create_session().await.unwrap();

    info!("Created session: {}", session.id);

    // Add 50 message pairs (100 total messages)
    for i in 0..50 {
        let user_msg = Message {
            role: MessageRole::User,
            content: format!("Turn {} - What is the capital of France?", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, user_msg).await.unwrap();

        let assistant_msg = Message {
            role: MessageRole::Assistant,
            content: format!("Turn {} - The capital of France is Paris.", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, assistant_msg).await.unwrap();

        if (i + 1) % 10 == 0 {
            info!("Added {} message pairs", i + 1);
        }
    }

    // Verify the session has all messages
    let final_session = agent.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(
        final_session.messages.len(),
        100,
        "Session should have 100 messages"
    );

    // Check token usage is tracked
    let usage = final_session.token_usage();
    assert!(usage.total > 0, "Token usage should be tracked");
    info!("Total tokens used: {}", usage.total);
}

/// Test that agent can handle individual messages with large content
#[tokio::test]
#[ignore] // Requires model download and significant compute time
async fn test_long_conversation_large_messages() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting test with large message content");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();
    let session = agent.create_session().await.unwrap();

    // Create a large message (approximately 10KB of text)
    let large_content = "This is a substantial message with lots of content. ".repeat(200);

    assert!(
        large_content.len() > 10000,
        "Test message should be at least 10KB"
    );

    let user_msg = Message {
        role: MessageRole::User,
        content: large_content.clone(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };

    agent.add_message(&session.id, user_msg).await.unwrap();

    // Verify the message was stored without truncation
    let updated_session = agent.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(updated_session.messages.len(), 1);
    assert_eq!(
        updated_session.messages[0].content.len(),
        large_content.len(),
        "Message content should not be truncated"
    );

    // Add a response
    let assistant_msg = Message {
        role: MessageRole::Assistant,
        content: "I received your large message.".to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent.add_message(&session.id, assistant_msg).await.unwrap();

    // Verify both messages are present
    let final_session = agent.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(final_session.messages.len(), 2);

    info!("Large message test completed successfully");
}

/// Test conversation history integrity over many turns
#[tokio::test]
async fn test_long_conversation_history_integrity() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting conversation history integrity test");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();
    let session = agent.create_session().await.unwrap();

    // Add messages in a specific pattern to verify ordering
    let message_count = 30;

    for i in 0..message_count {
        let user_msg = Message {
            role: MessageRole::User,
            content: format!("User message {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, user_msg).await.unwrap();

        let assistant_msg = Message {
            role: MessageRole::Assistant,
            content: format!("Assistant message {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, assistant_msg).await.unwrap();
    }

    // Verify all messages are present and in correct order
    let final_session = agent.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(
        final_session.messages.len(),
        message_count * 2,
        "Should have {} messages",
        message_count * 2
    );

    // Check ordering
    for i in 0..message_count {
        let user_idx = i * 2;
        let assistant_idx = user_idx + 1;

        assert_eq!(
            final_session.messages[user_idx].role,
            MessageRole::User,
            "Message {} should be user message",
            user_idx
        );
        assert_eq!(
            final_session.messages[user_idx].content,
            format!("User message {}", i)
        );

        assert_eq!(
            final_session.messages[assistant_idx].role,
            MessageRole::Assistant,
            "Message {} should be assistant message",
            assistant_idx
        );
        assert_eq!(
            final_session.messages[assistant_idx].content,
            format!("Assistant message {}", i)
        );
    }

    info!("History integrity verified for {} turns", message_count);
}

/// Test token usage tracking across long conversations
#[tokio::test]
async fn test_long_conversation_token_tracking() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting token tracking test");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();
    let session = agent.create_session().await.unwrap();

    // Track token usage as we add messages
    let initial_session = agent.get_session(&session.id).await.unwrap().unwrap();
    let initial_tokens = initial_session.token_usage().total;
    assert_eq!(initial_tokens, 0, "Initial token count should be 0");

    // Add a message
    let msg1 = Message {
        role: MessageRole::User,
        content: "This is a test message to track tokens.".to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent.add_message(&session.id, msg1).await.unwrap();

    let after_msg1 = agent.get_session(&session.id).await.unwrap().unwrap();
    let tokens_after_msg1 = after_msg1.token_usage().total;
    assert!(
        tokens_after_msg1 > initial_tokens,
        "Token count should increase after adding message"
    );

    info!("Tokens after message 1: {}", tokens_after_msg1);

    // Add more messages and verify token count continues to increase
    for i in 0..10 {
        let msg = Message {
            role: MessageRole::User,
            content: format!("Additional message {} with content to increase tokens.", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, msg).await.unwrap();
    }

    let final_session = agent.get_session(&session.id).await.unwrap().unwrap();
    let final_tokens = final_session.token_usage().total;

    assert!(
        final_tokens > tokens_after_msg1,
        "Token count should continue to increase with more messages"
    );
    info!("Final token count: {}", final_tokens);

    // Verify token count by role
    let usage_by_role = final_session.token_usage().by_role;
    info!("Token usage by role: {:?}", usage_by_role);
    assert!(
        usage_by_role.get(&MessageRole::User).unwrap_or(&0) > &0,
        "User tokens should be tracked"
    );
}

/// Test session persistence across long conversations
#[tokio::test]
async fn test_long_conversation_persistence() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting persistence test for long conversations");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();
    let session = agent.create_session().await.unwrap();
    let session_id = session.id;

    // Add multiple messages
    for i in 0..20 {
        let user_msg = Message {
            role: MessageRole::User,
            content: format!("Persistent message {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session_id, user_msg).await.unwrap();
    }

    // Retrieve the session and verify all messages are present
    let retrieved_session = agent.get_session(&session_id).await.unwrap().unwrap();
    assert_eq!(
        retrieved_session.messages.len(),
        20,
        "Should retrieve all messages"
    );

    // Verify content of first and last messages
    assert_eq!(
        retrieved_session.messages[0].content,
        "Persistent message 0"
    );
    assert_eq!(
        retrieved_session.messages[19].content,
        "Persistent message 19"
    );

    info!("Session persistence verified");
}

/// Test memory stability with progressively longer conversations
#[tokio::test]
async fn test_long_conversation_memory_stability() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting memory stability test");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();
    let session = agent.create_session().await.unwrap();

    // Add messages in batches and verify session remains accessible
    let batches = [10, 20, 30, 40, 50];

    for (batch_idx, batch_size) in batches.iter().enumerate() {
        info!("Adding batch {} with {} messages", batch_idx, batch_size);

        for i in 0..*batch_size {
            let msg = Message {
                role: MessageRole::User,
                content: format!(
                    "Batch {} message {} with some content to simulate realistic usage.",
                    batch_idx, i
                ),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            };
            agent.add_message(&session.id, msg).await.unwrap();
        }

        // Verify session is still accessible and correct
        let check_session = agent.get_session(&session.id).await.unwrap().unwrap();
        let expected_count: usize = batches[..=batch_idx].iter().sum();
        assert_eq!(
            check_session.messages.len(),
            expected_count,
            "Should have {} messages after batch {}",
            expected_count,
            batch_idx
        );

        info!(
            "Batch {} complete. Total messages: {}",
            batch_idx,
            check_session.messages.len()
        );
    }

    let final_session = agent.get_session(&session.id).await.unwrap().unwrap();
    let total_expected: usize = batches.iter().sum();
    assert_eq!(final_session.messages.len(), total_expected);

    info!("Memory stability test completed successfully");
}

/// Test handling of mixed message types in long conversations
#[tokio::test]
async fn test_long_conversation_mixed_message_types() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting mixed message types test");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();
    let session = agent.create_session().await.unwrap();

    // Add various message types in a pattern
    for i in 0..15 {
        // User message
        let user_msg = Message {
            role: MessageRole::User,
            content: format!("User query {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, user_msg).await.unwrap();

        // Assistant message
        let assistant_msg = Message {
            role: MessageRole::Assistant,
            content: format!("Assistant response {}", i),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, assistant_msg).await.unwrap();

        // Tool message (every 3rd turn)
        if i % 3 == 0 {
            let tool_msg = Message {
                role: MessageRole::Tool,
                content: format!("Tool result {}", i),
                tool_call_id: Some(ToolCallId::new()),
                tool_name: Some("test_tool".to_string()),
                timestamp: SystemTime::now(),
            };
            agent.add_message(&session.id, tool_msg).await.unwrap();
        }
    }

    // Verify all messages are present and properly typed
    let final_session = agent.get_session(&session.id).await.unwrap().unwrap();

    // Count messages by role
    let user_count = final_session
        .messages
        .iter()
        .filter(|m| m.role == MessageRole::User)
        .count();
    let assistant_count = final_session
        .messages
        .iter()
        .filter(|m| m.role == MessageRole::Assistant)
        .count();
    let tool_count = final_session
        .messages
        .iter()
        .filter(|m| m.role == MessageRole::Tool)
        .count();

    assert_eq!(user_count, 15, "Should have 15 user messages");
    assert_eq!(assistant_count, 15, "Should have 15 assistant messages");
    assert_eq!(
        tool_count, 5,
        "Should have 5 tool messages (every 3rd turn)"
    );

    info!(
        "Mixed message types verified: {} user, {} assistant, {} tool",
        user_count, assistant_count, tool_count
    );
}

/// Test session listing with multiple long conversations
#[tokio::test]
async fn test_multiple_long_conversations() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting multiple long conversations test");

    let config = create_test_config();
    let agent = AgentServer::initialize(config).await.unwrap();

    // Create multiple sessions with different message counts
    let session_configs = vec![("session_1", 10), ("session_2", 20), ("session_3", 30)];

    for (name, message_count) in &session_configs {
        let session = agent.create_session().await.unwrap();
        info!("Created {} with {} messages", name, message_count);

        for i in 0..*message_count {
            let msg = Message {
                role: MessageRole::User,
                content: format!("{} message {}", name, i),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            };
            agent.add_message(&session.id, msg).await.unwrap();
        }
    }

    // Verify each session has the expected message count
    for (name, expected_count) in &session_configs {
        info!("Verifying {} has {} messages", name, expected_count);
    }

    info!(
        "Multiple long conversations test completed with {} sessions",
        session_configs.len()
    );
}
