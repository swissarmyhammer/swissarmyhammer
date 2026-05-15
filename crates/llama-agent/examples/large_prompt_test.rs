//! Test case for reproducing the batch chunk decode error with large prompts
//!
//! This example demonstrates the issue described in GitHub issue where large prompts
//! that exceed the batch size cause "Failed to decode batch chunk: Decode Error -1: n_tokens == 0"
//!
//! The issue occurs when:
//! 1. A prompt is tokenized to more tokens than the batch_size setting
//! 2. The chunking logic incorrectly handles position tracking across chunks
//! 3. This causes llama.cpp to receive invalid batch position information

use llama_agent::{
    types::{
        AgentAPI, AgentConfig, GenerationRequest, Message, MessageRole, ModelConfig, ModelSource,
        QueueConfig, RetryConfig, SessionConfig,
    },
    AgentServer,
};
use std::time::SystemTime;
use tracing::{info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see the debug messages
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    info!("Testing large prompt batch processing with Qwen3 model");

    // Create agent configuration with the exact model from the issue
    // and a small batch size to trigger the chunking logic
    let config = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF".to_string(),
                filename: Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q4_K_XL.gguf".to_string()),
                folder: None,
            },
            batch_size: 512, // Small batch size to trigger chunking with large prompts
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: true,
            retry_config: RetryConfig::default(),
            debug: true, // Enable debug logging to see batch processing
        },
        queue_config: QueueConfig {
            max_queue_size: 100,
            worker_threads: 1,
        },
        mcp_servers: Vec::new(), // No MCP servers needed for this test
        session_config: SessionConfig::default(),
        parallel_execution_config: Default::default(),
    };

    info!("Initializing AgentServer (this will load the Qwen3 model)...");
    let agent = AgentServer::initialize(config).await?;
    info!("AgentServer initialized successfully");

    // Create a session for testing
    let session = agent.create_session().await?;
    info!("Created session: {}", session.id);

    // Create a very large system prompt that will definitely exceed 512 tokens
    let large_system_prompt = format!(
        r#"You are an expert software engineer and code reviewer. Your task is to provide comprehensive, detailed analysis of code and software engineering problems.

{}

When analyzing code, you should:
1. Review the overall architecture and design patterns
2. Identify potential bugs, security vulnerabilities, and performance issues
3. Check for code style consistency and adherence to best practices
4. Suggest improvements for maintainability and readability
5. Verify that error handling is robust and comprehensive
6. Ensure that the code follows SOLID principles and other software engineering best practices
7. Look for opportunities to reduce code duplication and improve modularity
8. Consider scalability and performance implications
9. Review test coverage and suggest additional test cases
10. Check for proper documentation and comments

Always provide specific examples and actionable recommendations. Be thorough in your analysis but also practical in your suggestions."#,
        // Add repetitive content to ensure we exceed 512 tokens
        (0..50).map(|i| format!("This is additional context line {} to increase the token count and ensure we trigger the batch size limit processing logic in the queue module.", i)).collect::<Vec<_>>().join(" ")
    );

    // Add the large system message
    let system_message = Message {
        role: MessageRole::System,
        content: large_system_prompt,
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent.add_message(&session.id, system_message).await?;

    // Add a user message that, combined with the system message, will definitely exceed batch size
    let user_message = Message {
        role: MessageRole::User,
        content: "Please analyze this code and provide detailed feedback:\n\nfn example_function() {\n    println!(\"Hello, world!\");\n}".to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent.add_message(&session.id, user_message).await?;

    // Test 1: Try batch processing
    info!("=== Testing Batch Processing ===");
    let batch_request = GenerationRequest::new(session.id)
        .with_max_tokens(50) // Small token count to focus on the prompt processing
        .with_temperature(0.1);

    match agent.generate(batch_request).await {
        Ok(response) => {
            info!("✅ Batch processing succeeded!");
            info!(
                "Generated {} tokens in {:?}",
                response.tokens_generated, response.generation_time
            );
            info!(
                "Response preview: {}",
                response
                    .generated_text
                    .chars()
                    .take(100)
                    .collect::<String>()
            );
        }
        Err(e) => {
            warn!("❌ Batch processing failed: {}", e);
            println!("Batch processing error: {}", e);
        }
    }

    // Test 2: Try streaming processing
    info!("=== Testing Streaming Processing ===");
    let streaming_request = GenerationRequest::new(session.id)
        .with_max_tokens(50)
        .with_temperature(0.1);

    match agent.submit_streaming_request(streaming_request).await {
        Ok(mut stream) => {
            info!("✅ Streaming processing started successfully!");
            let mut _token_count = 0;
            let mut generated_text = String::new();

            while let Some(chunk_result) = stream.recv().await {
                match chunk_result {
                    Ok(chunk) => {
                        if chunk.is_complete {
                            info!(
                                "✅ Streaming completed! Generated {} tokens",
                                chunk.token_count
                            );
                            break;
                        } else {
                            generated_text.push_str(&chunk.text);
                            _token_count = chunk.token_count;
                        }
                    }
                    Err(e) => {
                        warn!("❌ Streaming error: {}", e);
                        println!("Streaming error: {}", e);
                        break;
                    }
                }
            }

            if !generated_text.is_empty() {
                info!(
                    "Generated text preview: {}",
                    generated_text.chars().take(100).collect::<String>()
                );
            }
        }
        Err(e) => {
            warn!("❌ Streaming processing failed: {}", e);
            println!("Streaming processing error: {}", e);
        }
    }

    info!("Large prompt test completed");
    Ok(())
}
