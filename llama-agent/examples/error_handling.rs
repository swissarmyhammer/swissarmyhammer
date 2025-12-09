//! Error Handling and Recovery Examples
//!
//! This example demonstrates various error conditions that can occur when using
//! the llama-agent library and how to handle them gracefully:
//!
//! - Model loading failures
//! - Invalid configurations
//! - MCP server connection issues
//! - Tool execution failures
//! - Network timeouts and recovery
//! - Graceful degradation strategies

use llama_agent::{
    types::{
        AgentAPI, AgentConfig, FinishReason, GenerationRequest, MCPServerConfig, Message,
        MessageRole, ModelConfig, ModelSource, ParallelConfig, ProcessServerConfig, QueueConfig,
        RetryConfig, SessionConfig,
    },
    AgentServer,
};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to see error details
    tracing_subscriber::fmt::init();

    info!("Starting error handling and recovery examples");

    println!("Error Handling and Recovery Examples");
    println!("{}", "=".repeat(60));

    // Example 1: Invalid model configuration
    demonstrate_invalid_model_config().await?;

    // Example 2: MCP server failures
    demonstrate_mcp_server_failures().await?;

    // Example 3: Generation errors and recovery
    demonstrate_generation_errors().await?;

    // Example 4: Tool execution failures
    demonstrate_tool_failures().await?;

    // Example 5: Timeout handling
    demonstrate_timeout_handling().await?;

    // Example 6: Graceful degradation
    demonstrate_graceful_degradation().await?;

    println!("\n✓ All error handling examples completed");
    info!("Error handling examples completed");
    Ok(())
}

async fn demonstrate_invalid_model_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n1. Invalid Model Configuration Handling");
    println!("{}", "-".repeat(40));

    // Test 1: Invalid HuggingFace repo format
    println!("\nTest 1a: Invalid HuggingFace repo format");
    let invalid_hf_config = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "invalid-repo-format".to_string(), // Missing org/repo format
                filename: None,
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
        queue_config: QueueConfig::default(),
        mcp_servers: vec![],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    match <AgentServer as AgentAPI>::initialize(invalid_hf_config).await {
        Ok(_) => println!("❌ Should have failed with invalid HuggingFace repo"),
        Err(e) => println!("✓ Correctly caught invalid repo format: {}", e),
    }

    // Test 1b: Invalid local path
    println!("\nTest 1b: Invalid local model path");
    let invalid_local_config = AgentConfig {
        model: ModelConfig {
            source: ModelSource::Local {
                folder: PathBuf::from("/nonexistent/path"),
                filename: None,
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        },
        queue_config: QueueConfig::default(),
        mcp_servers: vec![],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    match <AgentServer as AgentAPI>::initialize(invalid_local_config).await {
        Ok(_) => println!("❌ Should have failed with invalid local path"),
        Err(e) => println!("✓ Correctly caught invalid local path: {}", e),
    }

    // Test 1c: Invalid batch size
    println!("\nTest 1c: Invalid batch size");
    let invalid_batch_config = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "microsoft/DialoGPT-medium".to_string(),
                filename: None,
                folder: None,
            },
            batch_size: 0, // Invalid batch size
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: true,
            retry_config: RetryConfig::default(),
            debug: false,
        },
        queue_config: QueueConfig::default(),
        mcp_servers: vec![],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    match <AgentServer as AgentAPI>::initialize(invalid_batch_config).await {
        Ok(_) => println!("❌ Should have failed with invalid batch size"),
        Err(e) => println!("✓ Correctly caught invalid batch size: {}", e),
    }

    Ok(())
}

async fn demonstrate_mcp_server_failures() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n2. MCP Server Failure Handling");
    println!("{}", "-".repeat(40));

    // Test with invalid MCP server commands
    let config_with_invalid_mcp = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "microsoft/DialoGPT-medium".to_string(),
                filename: None,
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
        queue_config: QueueConfig::default(),
        mcp_servers: vec![
            // Valid server (might work)
            MCPServerConfig::InProcess(ProcessServerConfig {
                name: "filesystem".to_string(),
                command: "npx".to_string(),
                args: vec![
                    "-y".to_string(),
                    "@modelcontextprotocol/server-filesystem".to_string(),
                ],
                timeout_secs: Some(30),
            }),
            // Invalid server command
            MCPServerConfig::InProcess(ProcessServerConfig {
                name: "invalid".to_string(),
                command: "nonexistent-command".to_string(),
                args: vec!["arg1".to_string()],
                timeout_secs: Some(10),
            }),
        ],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    println!("Attempting to initialize with invalid MCP servers...");
    match <AgentServer as AgentAPI>::initialize(config_with_invalid_mcp).await {
        Ok(agent) => {
            println!("✓ Agent initialized despite MCP failures (graceful degradation)");

            // Test tool discovery with partial MCP failures
            let mut session = agent.create_session().await?;
            match agent.discover_tools(&mut session).await {
                Ok(_) => {
                    println!(
                        "✓ Tool discovery succeeded with {} tools",
                        session.available_tools.len()
                    );
                    for tool in &session.available_tools {
                        println!("  - {}: {}", tool.name, tool.description);
                    }
                }
                Err(e) => {
                    println!("⚠ Tool discovery partially failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("⚠ Agent initialization failed with MCP errors: {}", e);
            println!("This might be expected if no MCP servers can be started");
        }
    }

    Ok(())
}

async fn demonstrate_generation_errors() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. Generation Error Handling");
    println!("{}", "-".repeat(40));

    // Create a minimal working configuration
    let config = AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: "microsoft/DialoGPT-medium".to_string(),
                filename: None,
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
        queue_config: QueueConfig {
            max_queue_size: 10,
            worker_threads: 1,
        },
        mcp_servers: vec![],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
    };

    println!("Attempting to initialize agent for generation error tests...");
    match <AgentServer as AgentAPI>::initialize(config).await {
        Ok(agent) => {
            println!("✓ Agent initialized successfully");

            // Test with problematic prompt
            let mut session = agent.create_session().await?;
            session.messages.push(Message {
                role: MessageRole::User,
                content: "Generate an extremely long response that might cause issues with memory or timeouts".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            });

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10000), // Very large token limit
                temperature: Some(2.0),  // Extreme temperature
                top_p: Some(1.0),
                stop_tokens: vec![],
                stopping_config: None,
            };

            match agent.generate(request).await {
                Ok(response) => {
                    println!("✓ Generation completed");
                    match &response.finish_reason {
                        FinishReason::Stopped(reason) if reason.starts_with("Error: ") => {
                            let error = &reason[7..]; // Remove "Error: " prefix
                            println!("  ⚠ Generation finished with error: {}", error);
                        }
                        FinishReason::Stopped(reason) if reason == "Maximum tokens reached" => {
                            println!("  ℹ Generation stopped due to token limit");
                        }
                        _ => {
                            println!(
                                "  ✓ Generation completed normally: {:?}",
                                response.finish_reason
                            );
                        }
                    }
                    println!("  Tokens generated: {}", response.tokens_generated);
                }
                Err(e) => {
                    println!("⚠ Generation failed as expected: {}", e);
                    println!("  This demonstrates error handling for generation failures");
                }
            }
        }
        Err(e) => {
            println!("⚠ Agent initialization failed: {}", e);
            println!("This is expected if the model cannot be loaded");
        }
    }

    Ok(())
}

async fn demonstrate_tool_failures() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n4. Tool Execution Failure Handling");
    println!("{}", "-".repeat(40));

    // This test would require a working agent with MCP servers
    // For now, we'll demonstrate the error handling patterns

    println!("Tool execution failures can occur due to:");
    println!("  • Tool not found in available tools");
    println!("  • Invalid tool arguments");
    println!("  • MCP server communication errors");
    println!("  • Tool execution timeouts");
    println!("  • Underlying system errors (file permissions, network, etc.)");

    println!("\nError handling strategies:");
    println!("  • ToolResult includes error field for graceful failure reporting");
    println!("  • Partial tool execution continues workflow with available results");
    println!("  • Tool call validation prevents invalid requests");
    println!("  • Retry mechanisms for transient failures");

    // Show conceptual error handling
    println!("\nConceptual tool error handling:");
    println!("```rust");
    println!("match agent.execute_tool(tool_call, &session).await {{");
    println!("    Ok(tool_result) => {{");
    println!("        if let Some(error) = &tool_result.error {{");
    println!("            // Tool executed but returned an error");
    println!("            warn!(\"Tool '{{}}' failed: {{}}\", tool_call.name, error);");
    println!("            // Continue with partial results");
    println!("        }} else {{");
    println!("            // Tool executed successfully");
    println!("            info!(\"Tool '{{}}' completed\", tool_call.name);");
    println!("        }}");
    println!("    }}");
    println!("    Err(agent_error) => {{");
    println!("        // Fatal error in tool execution system");
    println!("        error!(\"Tool execution system error: {{}}\", agent_error);");
    println!("        // Implement fallback or abort strategy");
    println!("    }}");
    println!("}}");
    println!("```");

    Ok(())
}

async fn demonstrate_timeout_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n5. Timeout Handling");
    println!("{}", "-".repeat(40));

    // Demonstrate different timeout scenarios
    println!("Timeout scenarios and handling:");

    println!("\n• Model loading timeout:");
    println!("  - Occurs when model download/loading takes too long");
    println!("  - Handled by: retry with exponential backoff, fallback models");

    println!("\n• Generation cancellation:");
    println!("  - Occurs when generation is cancelled by user");
    println!("  - Handled by: graceful cleanup and resource release");

    println!("\n• MCP server timeout:");
    println!("  - Occurs when tool execution exceeds timeout_secs");
    println!("  - Handled by: per-server timeouts, graceful degradation");

    println!("\n• Network timeout:");
    println!("  - Occurs during model download or MCP communication");
    println!("  - Handled by: retry logic, offline mode, cached resources");

    // Show configuration examples
    println!("\nConfiguration Examples:");
    println!("```rust");
    println!("QueueConfig {{");
    println!("    max_queue_size: 100,");
    println!("    worker_threads: 4,");
    println!("}}");
    println!();
    println!("MCPServerConfig {{");
    println!("    timeout_secs: Some(60), // 1 minute for tool execution");
    println!("    // ... other fields");
    println!("}}");
    println!("```");

    Ok(())
}

async fn demonstrate_graceful_degradation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n6. Graceful Degradation Strategies");
    println!("{}", "-".repeat(40));

    println!("Graceful degradation allows the system to continue operating");
    println!("with reduced functionality when components fail:");

    println!("\n• Model failures:");
    println!("  → Fallback to smaller/cached model");
    println!("  → Offline mode with pre-computed responses");
    println!("  → Error messages with helpful guidance");

    println!("\n• MCP server failures:");
    println!("  → Continue with available servers only");
    println!("  → Inform user about unavailable tools");
    println!("  → Provide manual alternatives");

    println!("\n• Tool execution failures:");
    println!("  → Return error in ToolResult, continue workflow");
    println!("  → Skip failed tools, process successful ones");
    println!("  → Suggest alternative approaches to user");

    println!("\n• Network failures:");
    println!("  → Use cached models and data");
    println!("  → Queue operations for later retry");
    println!("  → Inform user of connectivity status");

    // Demonstrate resilient configuration
    println!("\nResilient Configuration Example:");
    println!("```rust");
    println!("AgentConfig {{");
    println!("    // Multiple fallback options");
    println!("    model: ModelConfig {{");
    println!("        source: ModelSource::Local {{ // Prefer local for reliability");
    println!("            folder: PathBuf::from(\"./models/cached\"),");
    println!("            filename: Some(\"fallback-model.gguf\".to_string()),");
    println!("        }},");
    println!("        batch_size: 256, // Conservative batch size");
    println!("        use_hf_params: false, // Don't depend on network");
    println!("    }},");
    println!("    queue_config: QueueConfig {{");
    println!("        max_queue_size: 1000, // Large queue for resilience");
    println!("        worker_threads: 1, // Conservative threading");
    println!("    }},");
    println!("    // Only include essential MCP servers");
    println!("    mcp_servers: vec![essential_servers_only()],");
    println!("    session_config: SessionConfig {{");
    println!("        max_sessions: 100, // Reasonable limit");
    println!("    }},");
    println!("}}");
    println!("```");

    // Error recovery patterns
    println!("\nError Recovery Patterns:");
    println!("1. **Circuit Breaker**: Stop calling failing services temporarily");
    println!("2. **Bulkhead**: Isolate failures to prevent cascade");
    println!("3. **Retry with Backoff**: Retry failed operations with increasing delays");
    println!("4. **Fallback**: Use alternative implementations or cached data");
    println!("5. **Health Checks**: Monitor component health and route accordingly");

    // Demonstrate the retry functionality
    println!("\nDemonstrating retry with backoff:");
    let retry_result = retry_with_backoff(
        || async {
            // Simulate a flaky operation that eventually succeeds
            static mut COUNTER: i32 = 0;
            unsafe {
                COUNTER += 1;
                if COUNTER < 3 {
                    Err("Simulated transient error")
                } else {
                    Ok("Operation succeeded!")
                }
            }
        },
        5,
        Duration::from_millis(100),
    )
    .await;

    match retry_result {
        Ok(result) => println!("✓ Retry operation succeeded: {}", result),
        Err(e) => println!("⚠ Retry operation failed: {}", e),
    }

    Ok(())
}

/// Demonstrates how to implement retry logic for transient failures
/// This function shows practical retry patterns with exponential backoff
async fn retry_with_backoff<T, E, F, Fut>(
    mut operation: F,
    max_retries: usize,
    initial_delay: Duration,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut delay = initial_delay;

    for attempt in 0..max_retries {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt == max_retries - 1 => {
                error!("Operation failed after {} attempts: {}", max_retries, e);
                return Err(e);
            }
            Err(e) => {
                warn!(
                    "Attempt {} failed: {}, retrying in {:?}",
                    attempt + 1,
                    e,
                    delay
                );
                tokio::time::sleep(delay).await;
                delay = Duration::from_millis((delay.as_millis() * 2).min(30000) as u64);
                // Cap at 30s
            }
        }
    }

    unreachable!()
}
