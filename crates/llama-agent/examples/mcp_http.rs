//! HTTP MCP Server Integration Example
//!
//! This example demonstrates how to use HTTP-based MCP servers with the echo server.
//! It follows the same pattern as basic_usage.rs but uses an HTTP MCP server instead
//! of an InProcess server to show tool discovery and execution over HTTP.

use llama_agent::{
    types::{
        AgentAPI, AgentConfig, FinishReason, GenerationRequest, HttpServerConfig, MCPServerConfig,
        Message, MessageRole, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
        SessionConfig, StoppingConfig,
    },
    AgentServer,
};
use std::time::SystemTime;
use tracing::{error, info};

/// Creates the HTTP MCP server configuration for the echo server
fn create_echo_server_config() -> MCPServerConfig {
    MCPServerConfig::Http(HttpServerConfig {
        name: "echo-server".to_string(),
        url: "https://echo.mcp.inevitable.fyi/mcp".to_string(),
        timeout_secs: Some(30),
        sse_keep_alive_secs: Some(45),
        stateful_mode: true,
    })
}

/// HTTP MCP Server Integration Example
///
/// Demonstrates the complete HTTP MCP server workflow including:
/// - Configuring an HTTP-based MCP server (echo server)
/// - Model initialization and session creation
/// - Tool discovery from the HTTP MCP server
/// - Processing a user prompt that triggers tool usage
/// - Tool call detection and execution over HTTP
/// - Displaying results and statistics
///
/// This example follows the same pattern as `basic_usage.rs` but uses an HTTP MCP server
/// instead of an in-process server to showcase distributed tool execution.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    info!("Starting HTTP MCP server example with echo server");

    // Create agent configuration with HTTP MCP server
    let config = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF".to_string(),
                filename: Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q8_K_XL.gguf".to_string()),
                folder: None,
            },
            batch_size: 4096,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: true,
            retry_config: RetryConfig::default(),
            debug: false,
        },
        queue_config: QueueConfig {
            max_queue_size: 100,
            worker_threads: 1,
        },
        mcp_servers: vec![create_echo_server_config()],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    info!("Initializing AgentServer (this may take a while for model loading)...");
    println!("Connecting to HTTP MCP server: https://echo.mcp.inevitable.fyi/mcp");

    let agent = match AgentServer::initialize(config).await {
        Ok(agent) => {
            info!("AgentServer initialized successfully");
            agent
        }
        Err(e) => {
            println!("❌ Failed to initialize HTTP MCP server: {}", e);
            return Err(e.into());
        }
    };

    // Create a session with HTTP MCP server
    // Note: MCP servers are configured both in AgentConfig (for initialization) and on the session
    // (for runtime tool discovery and execution). This follows the same pattern as basic_usage.rs.
    let mut session = agent.create_session().await?;
    session.mcp_servers = vec![create_echo_server_config()];

    info!("Created session: {}", session.id);

    // Discover available tools from HTTP MCP server
    println!("Discovering tools from HTTP MCP server...");
    agent.discover_tools(&mut session).await?;

    if session.available_tools.is_empty() {
        println!("⚠ No tools discovered from HTTP server");
        return Ok(());
    }

    println!("Available tools from HTTP MCP server:");
    for tool in &session.available_tools {
        println!("  - {}: {}", tool.name, tool.description);
    }

    // Add a message that will trigger the echo tool
    let message = Message {
        role: MessageRole::User,
        content:
            "Can you echo the message 'Hello from HTTP MCP server!' using the available echo tool?"
                .to_string(),
        tool_call_id: None,
        tool_name: None,
        timestamp: SystemTime::now(),
    };
    agent.add_message(&session.id, message).await?;

    // Generate response with tool call detection
    let stopping_config = StoppingConfig {
        max_tokens: Some(200),
        eos_detection: true,
    };

    let request = GenerationRequest::new(session.id)
        .with_temperature(0.7)
        .with_top_p(0.9)
        .with_stopping_config(stopping_config);

    info!("Generating response...");
    let response = agent.generate(request).await?;

    // Handle the response and tool calls
    match &response.finish_reason {
        FinishReason::Stopped(reason) => match reason.as_str() {
            "Tool call detected" => {
                info!("HTTP MCP server tool call detected!");
                println!("✓ HTTP MCP server tool call detected!");
                println!("Generated text with tool calls:");
                println!("{}", response.generated_text);
            }
            "Maximum tokens reached" => {
                println!("Response (truncated due to token limit):");
                println!("{}", response.generated_text);
            }
            "Stop token detected" | "End of sequence token detected" => {
                println!("Response:");
                println!("{}", response.generated_text);
            }
            reason if reason.starts_with("Error: ") => {
                let err = &reason[7..];
                error!("Generation completed with error: {}", err);
                println!("❌ HTTP MCP tool call error: {}", err);
                println!("Response with partial text:");
                println!("{}", response.generated_text);
            }
            _ => {
                println!("Generation stopped: {}", reason);
                println!("Response:");
                println!("{}", response.generated_text);
            }
        },
    }

    // Display generation statistics
    println!("\nGeneration Statistics:");
    println!("  Tokens generated: {}", response.tokens_generated);
    println!("  Time taken: {:?}", response.generation_time);
    println!("  Finish reason: {:?}", response.finish_reason);

    // Show session summary
    println!("\nHTTP MCP Session Summary:");
    println!("  Session ID: {}", session.id);
    println!("  Total messages: {}", session.messages.len());
    println!("  Available tools: {}", session.available_tools.len());
    println!("  HTTP server: https://echo.mcp.inevitable.fyi/mcp");

    info!("HTTP MCP server example completed successfully");
    Ok(())
}
