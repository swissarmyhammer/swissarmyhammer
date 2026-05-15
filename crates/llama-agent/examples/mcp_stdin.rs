//! Basic usage example from the specification
//!
//! This example demonstrates the complete system functionality as outlined in
//! specifications/index.md lines 605-709. It shows:
//!
//! - AgentConfig setup with HuggingFace model loading
//! - Session creation with MCP server configuration
//! - Tool discovery and integration
//! - User message processing with tool calls
//! - Tool execution and result integration
//! - Follow-up generation with tool results

use llama_agent::{
    types::{
        AgentAPI, AgentConfig, FinishReason, GenerationRequest, MCPServerConfig, Message,
        MessageRole, ModelConfig, ModelSource, ParallelConfig, ProcessServerConfig, QueueConfig,
        RetryConfig, SessionConfig, StoppingConfig,
    },
    AgentServer,
};
use std::time::SystemTime;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging with debug level
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting basic usage example");

    // Create agent configuration exactly as shown in the specification
    let config = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF".to_string(),
                filename: Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf".to_string()),
                folder: None,
            },
            batch_size: 4096,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: true, // Use HuggingFace generation_config.json
            retry_config: RetryConfig::default(),
            debug: false,
        },
        queue_config: QueueConfig {
            max_queue_size: 100,
            worker_threads: 1,
        },
        mcp_servers: vec![MCPServerConfig::InProcess(ProcessServerConfig {
            name: "filesystem".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
                ".".to_string(), // Current directory
            ],
            timeout_secs: None,
        })],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    info!("Initializing AgentServer (this may take a while for model loading)...");
    let agent = AgentServer::initialize(config).await?;
    info!("AgentServer initialized successfully");

    // Create a session with MCP servers
    let mut session = agent.create_session().await?;
    session.mcp_servers = vec![MCPServerConfig::InProcess(ProcessServerConfig {
        name: "filesystem".to_string(),
        command: "npx".to_string(),
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
            ".".to_string(), // Current directory
        ],
        timeout_secs: None,
    })];

    info!("Created session: {}", session.id);

    // Discover available tools from MCP servers
    agent.discover_tools(&mut session).await?;
    info!("Available tools: {:#?}", session.available_tools);

    for tool in &session.available_tools {
        println!("  - {}: {}", tool.name, tool.description);
    }

    // Add a message that might trigger tool use
    let message = Message {
        role: MessageRole::User,
        content: "Can you list the files in the current directory?".to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent.add_message(&session.id, message).await?;

    // Generate response
    // Create generation request with explicit stopping configuration
    let stopping_config = StoppingConfig {
        max_tokens: Some(100),
        eos_detection: true,
    };

    let request = GenerationRequest::new(session.id)
        .with_temperature(0.7)
        .with_top_p(0.9)
        .with_stopping_config(stopping_config);

    info!("Generating response...");
    let response = agent.generate(request).await?;

    // Check if the response includes tool calls
    match &response.finish_reason {
        FinishReason::Stopped(reason) => {
            match reason.as_str() {
                "Tool call detected" => {
                    info!("Model wants to call tools!");
                    println!("Model wants to call tools!");

                    // Extract tool calls from the generated text
                    // Note: The ChatTemplateEngine is used internally by AgentServer.generate()
                    // The tool call extraction and execution is handled automatically in the generate() method
                    // This example shows the conceptual flow, but the actual implementation is handled internally

                    println!("Generated text with tool calls:");
                    println!("{}", response.generated_text);

                    // The tool calls have already been processed by the generate() method
                    // and the response includes the final result after tool execution
                }
                "Maximum tokens reached" => {
                    println!("Response (truncated due to token limit):");
                    println!("{}", response.generated_text);
                }
                "Stop token detected" => {
                    println!("Response:");
                    println!("{}", response.generated_text);
                }
                "End of sequence token detected" => {
                    println!("Response:");
                    println!("{}", response.generated_text);
                }
                reason if reason.starts_with("Error: ") => {
                    let err = &reason[7..]; // Remove "Error: " prefix
                    error!("Generation completed with error: {}", err);
                    println!("❌ Generation completed with error: {}", err);
                    println!("Response with partial text:");
                    println!("{}", response.generated_text);
                }
                _ => {
                    println!("Generation stopped: {}", reason);
                    println!("Response:");
                    println!("{}", response.generated_text);
                }
            }
        }
    }

    // Display generation statistics
    println!("\nGeneration Statistics:");
    println!("  Tokens generated: {}", response.tokens_generated);
    println!("  Time taken: {:?}", response.generation_time);
    println!("  Finish reason: {:?}", response.finish_reason);

    info!("Basic usage example completed successfully");
    Ok(())
}
