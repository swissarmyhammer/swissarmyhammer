//! Advanced session compaction example
//!
//! Demonstrates advanced compaction features including:
//!
//! - Custom prompts for domain-specific summarization
//! - Auto-compaction across multiple sessions
//! - Batch compaction operations
//! - Monitoring and reporting compaction effectiveness
//! - Error handling and recovery strategies

use llama_agent::{
    types::{
        AgentAPI, AgentConfig, CompactionConfig, CompactionPrompt, Message, MessageRole,
        ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
        SessionId,
    },
    AgentServer,
};
use std::time::SystemTime;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with detailed output
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting advanced compaction example");

    // Create custom compaction prompt for technical conversations
    let technical_prompt = CompactionPrompt::custom(
        "You are an expert at creating concise summaries of technical conversations. Focus on preserving code snippets, technical decisions, implementation details, and architectural considerations. Maintain technical accuracy while reducing verbosity.".to_string(),
        "Create a comprehensive technical summary of this conversation, preserving all code examples, technical details, and implementation decisions:\n\n{conversation_history}\n\n**Technical Summary:**".to_string(),
    )?;

    info!("Created custom technical compaction prompt");

    // Configure advanced compaction settings
    let advanced_compaction_config = CompactionConfig {
        threshold: 0.75, // Compact at 75% of context window

        preserve_recent: 2, // Keep 2 most recent messages for continuity
        custom_prompt: Some(technical_prompt),
    };

    // Initialize agent with compaction-aware configuration
    let config = AgentConfig {
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
    };

    let agent = AgentServer::initialize(config).await?;
    info!("Agent initialized with advanced configuration");

    // Create multiple sessions for batch demonstration
    info!("Creating multiple sessions for batch compaction testing...");
    let mut session_ids = Vec::new();

    for i in 0..3 {
        let session = agent.create_session().await?;
        session_ids.push(session.id);

        info!("Created session {}: {}", i + 1, session.id);

        // Simulate different types of technical conversations
        let conversation_themes = [
            ("architecture", "system design", "scalability patterns"),
            ("debugging", "error analysis", "troubleshooting steps"),
            ("implementation", "code review", "best practices"),
        ];

        let theme = conversation_themes[i];

        // Add substantial conversation content
        for j in 0..20 {
            let user_msg = Message {
                role: MessageRole::User,
                content: format!(
                    "User message {} about {}: I need help with {} and {}. Can you explain the concepts, provide code examples, and discuss implementation strategies? This is particularly important for {} scenarios.",
                    j + 1, theme.0, theme.1, theme.2, theme.0
                ),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            };
            agent.add_message(&session.id, user_msg).await?;

            let assistant_msg = Message {
                role: MessageRole::Assistant,
                content: format!(
                    "Assistant response {} for {}: Here's a comprehensive explanation of {}. Let me provide detailed code examples:\n\n```rust\n// Example implementation for {}\nfn example_function() {{\n    // Implementation details here\n    println!(\"Handling {} with {}\");\n}}\n```\n\nKey considerations for {} include performance optimization, error handling, and maintainability. The {} approach is particularly effective when dealing with {} requirements.",
                    j + 1, theme.0, theme.1, theme.2, theme.1, theme.2, theme.0, theme.1, theme.2
                ),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            };
            agent.add_message(&session.id, assistant_msg).await?;
        }

        // Check session statistics
        let session = agent.get_session(&session.id).await?.unwrap();
        let usage = session.token_usage();
        info!(
            "Session {} statistics: {} messages, {} tokens",
            i + 1,
            session.messages.len(),
            usage.total
        );
    }

    info!(
        "All sessions created with {} total sessions",
        session_ids.len()
    );

    // Demonstrate individual session compaction with custom prompt
    info!("Performing individual compaction with technical prompt...");
    let first_session_id = &session_ids[0];

    if agent
        .should_compact_session(first_session_id, &advanced_compaction_config)
        .await?
    {
        let result = agent
            .compact_session(first_session_id, Some(advanced_compaction_config.clone()))
            .await?;

        info!("Individual compaction results:");
        info!("  Session: {}", first_session_id);
        info!("  Original messages: {}", result.original_messages);
        info!("  Original tokens: {}", result.original_tokens);
        info!("  Compressed tokens: {}", result.compressed_tokens);
        info!(
            "  Compression efficiency: {:.1}%",
            (1.0 - result.compression_ratio) * 100.0
        );
        info!(
            "  Space saved: {} tokens",
            result.original_tokens - result.compressed_tokens
        );

        // Examine the custom summary
        let compacted_session = agent.get_session(first_session_id).await?.unwrap();
        if let Some(summary_msg) = compacted_session.messages.first() {
            info!("Technical summary preview (first 300 chars):");
            let preview = if summary_msg.content.len() > 300 {
                format!("{}...", &summary_msg.content[..300])
            } else {
                summary_msg.content.clone()
            };
            info!("  {}", preview);
        }
    } else {
        warn!("First session doesn't meet compaction criteria yet");
    }

    // Demonstrate batch auto-compaction
    info!("Performing batch auto-compaction across all sessions...");

    // Use a lower threshold for batch demonstration
    let batch_config = CompactionConfig {
        threshold: 0.6, // Lower threshold to ensure compaction

        preserve_recent: 1,  // Minimal preservation for demo
        custom_prompt: None, // Use default prompt for comparison
    };

    let summary = agent.auto_compact_sessions(&batch_config).await?;

    info!("Auto-compaction batch results:");
    info!(
        "  Total sessions processed: {}",
        summary.total_sessions_processed
    );
    info!(
        "  Successful compactions: {}",
        summary.successful_compactions
    );
    info!(
        "  Failed compactions: {}",
        summary.total_sessions_processed - summary.successful_compactions
    );
    info!(
        "  Total messages compressed: {}",
        summary.total_messages_compressed
    );
    info!("  Total tokens saved: {}", summary.total_tokens_saved);
    info!(
        "  Average compression ratio: {:.1}%",
        (1.0 - summary.average_compression_ratio) * 100.0
    );

    // Monitor compaction effectiveness
    info!("Monitoring post-compaction session states...");
    for (i, session_id) in session_ids.iter().enumerate() {
        let session = agent.get_session(session_id).await?.unwrap();
        let current_usage = session.token_usage();

        info!("Session {} post-compaction:", i + 1);
        info!("  Messages: {}", session.messages.len());
        info!("  Current tokens: {}", current_usage.total);
        info!(
            "  Compaction history entries: {}",
            session.compaction_history.len()
        );

        // Show compaction history if available
        for (idx, metadata) in session.compaction_history.iter().enumerate() {
            info!(
                "    Compaction {}: {} -> {} tokens ({:.1}% reduction)",
                idx + 1,
                metadata.original_token_count,
                metadata.compressed_token_count,
                (1.0 - metadata.compression_ratio) * 100.0
            );
        }
    }

    // Demonstrate error handling and recovery
    info!("Testing compaction error handling...");

    // Try to compact a non-existent session
    let fake_session_id = SessionId::new();
    match agent.compact_session(&fake_session_id, None).await {
        Ok(_) => warn!("Unexpected success for invalid session"),
        Err(e) => info!("Expected error for invalid session: {}", e),
    }

    // Test with invalid configuration
    let invalid_config = CompactionConfig {
        threshold: -0.5, // Invalid threshold

        preserve_recent: 0,
        custom_prompt: None,
    };

    match invalid_config.validate() {
        Ok(_) => warn!("Unexpected validation success for invalid config"),
        Err(e) => info!("Expected validation error: {}", e),
    }

    // Demonstrate continued conversation after compaction
    info!("Testing conversation continuity after compaction...");

    let test_session_id = &session_ids[1];
    let new_user_msg = Message {
        role: MessageRole::User,
        content: "Based on our previous discussion, can you provide a quick summary of the key technical points we covered?".to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };

    agent.add_message(test_session_id, new_user_msg).await?;

    let final_session = agent.get_session(test_session_id).await?.unwrap();
    info!("Post-compaction conversation continuity:");
    info!("  Final message count: {}", final_session.messages.len());
    info!("  Final token count: {}", final_session.token_usage().total);

    // Performance monitoring
    let total_original_tokens: usize = summary.total_messages_compressed * 50; // Estimated
    let efficiency_ratio = if total_original_tokens > 0 {
        summary.total_tokens_saved as f32 / total_original_tokens as f32
    } else {
        0.0
    };

    info!("Overall compaction performance metrics:");
    info!(
        "  Processing efficiency: {:.1}% token reduction",
        efficiency_ratio * 100.0
    );
    info!(
        "  Memory savings: {} tokens freed",
        summary.total_tokens_saved
    );
    info!(
        "  Operation success rate: {:.1}%",
        (summary.successful_compactions as f32 / summary.total_sessions_processed as f32) * 100.0
    );

    info!("Advanced compaction example completed successfully");
    Ok(())
}
