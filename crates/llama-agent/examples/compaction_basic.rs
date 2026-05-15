//! Basic session compaction example
//!
//! Demonstrates how to use session compaction to manage long conversations.
//! This example shows:
//!
//! - Creating a session and adding multiple messages
//! - Checking token usage and compaction criteria
//! - Performing manual compaction with configuration
//! - Observing compression results and preserved messages

use llama_agent::{
    types::{
        AgentAPI, AgentConfig, CompactionConfig, Message, MessageRole, ModelConfig, ModelSource,
        ParallelConfig, QueueConfig, RetryConfig, SessionConfig,
    },
    AgentServer,
};
use std::time::SystemTime;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting basic compaction example");

    // Initialize the agent with a lightweight model for demonstration
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
    info!("Agent initialized successfully");

    // Create a session and add many messages to simulate a long conversation
    let session = agent.create_session().await?;
    info!("Created session: {}", session.id);

    info!("Creating a long conversation with multiple messages...");
    for i in 0..25 {
        // Add user message
        let user_msg = Message {
            role: MessageRole::User,
            content: format!(
                "This is user message {} with substantial content to increase token usage. I'm asking about various technical topics, implementation details, and complex scenarios that require detailed responses.",
                i
            ),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, user_msg).await?;

        // Add assistant response
        let assistant_msg = Message {
            role: MessageRole::Assistant,
            content: format!(
                "This is assistant response {} providing detailed information and comprehensive explanations. I'm covering technical concepts, code examples, best practices, troubleshooting steps, and implementation guidance to provide thorough assistance.",
                i
            ),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, assistant_msg).await?;

        if i % 5 == 0 {
            info!("Added {} message pairs...", i + 1);
        }
    }

    // Check current token usage
    let updated_session = agent.get_session(&session.id).await?.unwrap();
    let usage = updated_session.token_usage();
    info!("Current conversation statistics:");
    info!("  Total messages: {}", updated_session.messages.len());
    info!("  Total tokens: {}", usage.total);
    info!("  Tokens by role: {:?}", usage.by_role);

    // Configure compaction with a low context limit for demonstration
    let compaction_config = CompactionConfig {
        threshold: 0.6, // Compact at 60% of context window (lower for demo)

        preserve_recent: 3,  // Keep 3 most recent messages for continuity
        custom_prompt: None, // Use default summarization prompt
    };

    info!("Checking if compaction is needed...");
    let should_compact = agent
        .should_compact_session(&session.id, &compaction_config)
        .await?;

    if should_compact {
        info!("Session needs compaction - performing compaction...");

        match agent
            .compact_session(&session.id, Some(compaction_config))
            .await
        {
            Ok(result) => {
                info!("Compaction completed successfully!");
                info!("Compaction results:");
                info!("  Original messages: {}", result.original_messages);
                info!("  Original tokens: {}", result.original_tokens);
                info!("  Compressed tokens: {}", result.compressed_tokens);
                info!(
                    "  Compression ratio: {:.1}%",
                    result.compression_ratio * 100.0
                );
                info!(
                    "  Tokens saved: {}",
                    result.original_tokens - result.compressed_tokens
                );

                // Verify the session is now compacted
                let compacted_session = agent.get_session(&session.id).await?.unwrap();
                info!("Post-compaction statistics:");
                info!(
                    "  Messages after compaction: {}",
                    compacted_session.messages.len()
                );
                info!(
                    "  New token count: {}",
                    compacted_session.token_usage().total
                );

                // Show the generated summary
                if let Some(summary_msg) = compacted_session.messages.first() {
                    info!("Generated summary (first 200 chars):");
                    let preview = if summary_msg.content.len() > 200 {
                        format!("{}...", &summary_msg.content[..200])
                    } else {
                        summary_msg.content.clone()
                    };
                    info!("  {}", preview);
                }

                // Show preserved recent messages
                if compacted_session.messages.len() > 1 {
                    info!(
                        "Preserved recent messages: {}",
                        compacted_session.messages.len() - 1
                    );
                }
            }
            Err(e) => {
                warn!("Compaction failed: {}", e);
                info!("This could be due to various factors like insufficient model resources, token limits, or configuration issues.");
            }
        }
    } else {
        warn!("Session does not need compaction yet");
        info!(
            "Current usage: {} tokens, threshold: {} ({}%)",
            usage.total,
            compaction_config.threshold,
            compaction_config.threshold * 100.0
        );
    }

    // Demonstrate adding more messages after compaction
    if should_compact {
        info!("Adding new messages to continue the conversation...");

        let new_user_msg = Message {
            role: MessageRole::User,
            content: "Can you help me understand what we discussed earlier?".to_string(),
            tool_call_id: None,
            tool_name: None,
            timestamp: SystemTime::now(),
        };
        agent.add_message(&session.id, new_user_msg).await?;

        let final_session = agent.get_session(&session.id).await?.unwrap();
        info!(
            "Final session has {} messages and {} tokens",
            final_session.messages.len(),
            final_session.token_usage().total
        );
    }

    info!("Basic compaction example completed successfully");
    Ok(())
}
