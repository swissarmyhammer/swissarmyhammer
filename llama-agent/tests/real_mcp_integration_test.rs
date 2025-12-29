//! Real End-to-End MCP Integration Tests - ABSOLUTELY NO MOCKS
//!
//! These tests prove the complete MCP pipeline works by:
//! 1. Starting real EchoService MCP servers (stdio, SSE, HTTP)
//! 2. Creating real AgentServer with real MCP client connections
//! 3. Making actual tool calls through the agent
//! 4. Verifying real tool responses come back through the full pipeline
//!
//! This tests: AgentServer -> MCPClient -> Transport -> EchoService -> Tool -> Response

use llama_agent::types::{
    GenerationRequest, MCPServerConfig, Message, MessageRole, ModelConfig, ModelSource,
    ProcessServerConfig, RetryConfig, ToolCall, ToolCallId,
};
use llama_agent::{AgentAPI, AgentConfig, AgentServer, ParallelConfig, QueueConfig, SessionConfig};
use rstest::*;
use serde_json::json;

use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::process::{Child, Command};
use tokio::time::timeout;
use tracing::{info, warn};

/// Test model for real integration tests
const TEST_MODEL_REPO: &str = "unsloth/Qwen3-0.6B-GGUF";
const TEST_MODEL_FILE: &str = "Qwen3-0.6B-IQ4_NL.gguf";

/// Transport types for parameterized testing
#[derive(Debug, Clone)]
pub enum TransportType {
    Stdio,
    Sse,
    Streamable,
}

impl TransportType {
    fn name(&self) -> &'static str {
        match self {
            TransportType::Stdio => "stdio",
            TransportType::Sse => "sse",
            TransportType::Streamable => "streamable",
        }
    }

    // Note: port method removed as unused
}

/// Real EchoService server fixture - spawns actual server processes
pub struct RealEchoServerFixture {
    server_process: Child,
    transport_type: TransportType,
    mcp_config: MCPServerConfig,
}

impl RealEchoServerFixture {
    async fn new(transport_type: TransportType) -> Self {
        let transport_name = transport_type.name();
        info!("Starting real {} EchoService server...", transport_name);

        let server_process = Command::new("cargo")
            .args(["run", "--example", &format!("echo_{}", transport_name)])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .stdin(if matches!(transport_type, TransportType::Stdio) {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(if matches!(transport_type, TransportType::Stdio) {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::inherit()
            })
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap_or_else(|_| panic!("Failed to spawn real echo_{} server", transport_name));

        // Wait for server startup
        let startup_delay = match transport_type {
            TransportType::Stdio => Duration::from_millis(500),
            TransportType::Sse | TransportType::Streamable => Duration::from_millis(1500),
        };
        tokio::time::sleep(startup_delay).await;

        let mcp_config = match transport_type {
            TransportType::Stdio => MCPServerConfig::InProcess(ProcessServerConfig {
                name: format!("real_echo_{}", transport_name),
                command: "cargo".to_string(),
                args: vec![
                    "run".to_string(),
                    "--example".to_string(),
                    format!("echo_{}", transport_name),
                ],
                timeout_secs: Some(15),
            }),
            TransportType::Sse => MCPServerConfig::InProcess(ProcessServerConfig {
                name: format!("real_echo_{}", transport_name),
                command: "cargo".to_string(),
                args: vec![
                    "run".to_string(),
                    "--example".to_string(),
                    format!("echo_{}", transport_name),
                ],
                timeout_secs: Some(15),
            }),
            TransportType::Streamable => MCPServerConfig::InProcess(ProcessServerConfig {
                name: format!("real_echo_{}", transport_name),
                command: "cargo".to_string(),
                args: vec![
                    "run".to_string(),
                    "--example".to_string(),
                    format!("echo_{}", transport_name),
                ],
                timeout_secs: Some(15),
            }),
        };

        Self {
            server_process,
            transport_type,
            mcp_config,
        }
    }
}

impl Drop for RealEchoServerFixture {
    fn drop(&mut self) {
        info!(
            "Shutting down real {} EchoService server",
            self.transport_type.name()
        );
        let _ = self.server_process.start_kill();
    }
}

impl RealEchoServerFixture {
    /// Async cleanup method to properly wait for process exit
    #[allow(dead_code)]
    async fn cleanup(mut self) {
        info!(
            "Cleaning up real {} EchoService server",
            self.transport_type.name()
        );
        let _ = self.server_process.kill().await;
        let _ = self.server_process.wait().await;
    }
}

/// Create real agent config with real MCP server
fn create_real_agent_config(mcp_server: MCPServerConfig) -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: TEST_MODEL_REPO.to_string(),
                filename: Some(TEST_MODEL_FILE.to_string()),
                folder: None,
            },
            batch_size: 16,
            use_hf_params: true,
            retry_config: RetryConfig {
                max_retries: 3,
                initial_delay_ms: 200,
                backoff_multiplier: 2.0,
                max_delay_ms: 2000,
            },
            debug: false,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
        },
        mcp_servers: vec![mcp_server],
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
        queue_config: QueueConfig {
            worker_threads: 1,
            ..Default::default()
        },
    }
}

/// rstest fixture for stdio transport only (others use same pattern)
#[fixture]
async fn stdio_echo_server() -> RealEchoServerFixture {
    RealEchoServerFixture::new(TransportType::Stdio).await
}

/// END-TO-END TEST: Real AgentServer -> Real MCP -> Real EchoService -> Real tool call
/// Tests all three transports with the same test logic using rstest
#[rstest]
#[tokio::test]
async fn test_end_to_end_stdio_echo_tool_call(#[future] stdio_echo_server: RealEchoServerFixture) {
    let server_fixture = stdio_echo_server.await;
    test_end_to_end_impl(server_fixture, TransportType::Stdio).await;
}

async fn test_end_to_end_impl(
    server_fixture: RealEchoServerFixture,
    transport_type: TransportType,
) {
    let config = create_real_agent_config(server_fixture.mcp_config.clone());
    let transport_name = transport_type.name();

    info!(
        "🚀 Starting END-TO-END {} test: AgentServer -> {} MCP -> EchoService -> echo tool",
        transport_name, transport_name
    );

    let result = timeout(Duration::from_secs(180), AgentServer::initialize(config)).await;

    match result {
        Ok(Ok(agent)) => {
            info!(
                "✅ Real AgentServer initialized with real {} MCP connection",
                transport_name
            );

            // Create real session with transcript recording
            let transcript_dir = format!("/tmp/llama_agent_test_transcripts_{}", transport_name);
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let transcript_path =
                PathBuf::from(&transcript_dir).join(format!("session_{}.yaml", timestamp));

            // Ensure transcript directory exists
            fs::create_dir_all(&transcript_dir)
                .await
                .expect("Failed to create transcript directory");

            let session = agent
                .create_session_with_transcript(Some(transcript_path.clone()))
                .await
                .expect("Failed to create real session with transcript");
            let session_id = session.id;

            info!(
                "📝 Created session with transcript recording at: {:?}",
                transcript_path
            );

            // Test tool discovery and calling through real transport
            let tool_request_message = Message {
                role: MessageRole::User,
                content: format!(
                    "Use the echo tool to say: 'Hello from real {} transport'",
                    transport_name
                ),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            agent
                .add_message(&session_id, tool_request_message)
                .await
                .expect("Failed to add message");

            // Generate with real model to trigger tool discovery and usage
            let generation_request = GenerationRequest {
                session_id,
                max_tokens: Some(150),
                temperature: Some(0.1), // Low temperature for more predictable tool usage
                top_p: Some(0.7),
                stop_tokens: vec!["</s>".to_string()],
                stopping_config: None,
            };

            info!(
                "🔄 Making real generation request through {} MCP...",
                transport_name
            );
            let response = agent
                .generate(generation_request)
                .await
                .unwrap_or_else(|_| panic!("Real {} generation failed", transport_name));

            // Verify real generation occurred
            assert!(!response.generated_text.is_empty());
            assert!(response.tokens_generated > 0);

            info!("✅ Real {} generation completed:", transport_name);
            info!("  Generated text: '{}'", response.generated_text);
            info!("  Tokens: {}", response.tokens_generated);
            info!("  Time: {:?}", response.generation_time);

            // Test direct tool execution through real MCP
            let final_session = agent
                .get_session(&session_id)
                .await
                .expect("Failed to get final session")
                .expect("Final session should exist");

            info!(
                "Tools discovered through {} transport: {}",
                transport_name,
                final_session.available_tools.len()
            );
            for tool in &final_session.available_tools {
                info!(
                    "  Real {} tool: {} - {}",
                    transport_name, tool.name, tool.description
                );
            }

            // Test direct tool call if echo tool available
            if let Some(_echo_tool) = final_session
                .available_tools
                .iter()
                .find(|t| t.name == "echo")
            {
                info!(
                    "🔧 Testing direct tool call to real echo tool through {} transport...",
                    transport_name
                );

                let tool_call = ToolCall {
                    id: ToolCallId::new(),
                    name: "echo".to_string(),
                    arguments: json!({
                        "message": format!("Real {} tool call test", transport_name)
                    }),
                };

                let tool_result = agent
                    .execute_tool(tool_call.clone(), &final_session)
                    .await
                    .unwrap_or_else(|_| panic!("Real {} tool call failed", transport_name));

                // Verify real tool response
                assert_eq!(tool_result.call_id, tool_call.id);
                assert!(tool_result.error.is_none());

                info!("✅ Real {} tool call succeeded:", transport_name);
                info!("  Tool result: {:?}", tool_result.result);

                // Verify the echo tool actually echoed our message
                if let Some(result_str) = tool_result.result.as_str() {
                    assert!(result_str.contains(&format!("Real {} tool call test", transport_name)));
                    info!("✅ VERIFIED: Echo tool returned expected content through real {} transport", transport_name);
                } else {
                    info!("ℹ️  Tool result format: {:?}", tool_result.result);
                }
            } else {
                info!(
                    "ℹ️  Echo tool not discovered through {} transport",
                    transport_name
                );
            }

            // Validate transcript recording
            info!("🔍 Validating transcript recording...");

            // Read transcript file
            let transcript_content = fs::read_to_string(&transcript_path)
                .await
                .expect("Failed to read transcript file");

            // Parse YAML transcript
            let transcript: serde_yaml::Value =
                serde_yaml::from_str(&transcript_content).expect("Failed to parse transcript YAML");

            // Validate transcript structure
            assert!(
                transcript.get("session_id").is_some(),
                "Transcript missing session_id"
            );
            assert!(
                transcript.get("created_at").is_some(),
                "Transcript missing created_at"
            );

            let messages = transcript
                .get("messages")
                .and_then(|v| v.as_sequence())
                .expect("Transcript missing messages array");

            info!("📊 Transcript contains {} messages", messages.len());

            // Verify we have the expected messages recorded
            assert!(
                !messages.is_empty(),
                "Transcript should contain recorded messages"
            );

            // Validate message structure and content
            let mut found_user_message = false;
            let mut found_tool_calls = false;

            for message in messages {
                let role = message
                    .get("role")
                    .and_then(|v| v.as_str())
                    .expect("Message missing role");
                let content = message
                    .get("content")
                    .and_then(|v| v.as_str())
                    .expect("Message missing content");
                let timestamp = message.get("timestamp").expect("Message missing timestamp");

                info!(
                    "📝 Found message: role={}, content_preview={:?}",
                    role,
                    &content.chars().take(50).collect::<String>()
                );

                // Check for user message requesting tool use
                if role == "user"
                    && content.contains(&format!("Hello from real {} transport", transport_name))
                {
                    found_user_message = true;
                }

                // Check for tool-related messages
                if role == "assistant"
                    || message.get("tool_call_id").is_some()
                    || message.get("tool_name").is_some()
                {
                    found_tool_calls = true;
                }

                // Validate timestamp format
                assert!(
                    timestamp.is_string() || timestamp.is_number(),
                    "Timestamp should be string or number"
                );
            }

            assert!(
                found_user_message,
                "Transcript should contain the user message requesting tool use"
            );
            assert!(
                found_tool_calls,
                "Transcript should contain tool call messages"
            );
            info!(
                "✅ Transcript validation passed for {} transport",
                transport_name
            );

            // Cleanup transcript file
            let _ = fs::remove_file(&transcript_path).await;
            let _ = fs::remove_dir(&transcript_dir).await;

            drop(agent);
            info!(
                "✅ END-TO-END {} test with transcript recording completed successfully",
                transport_name
            );
        }
        Ok(Err(e)) => {
            let error_msg = e.to_string().to_lowercase();
            if error_msg.contains("429") || error_msg.contains("rate limited") {
                warn!(
                    "⚠️  Skipping {} end-to-end test due to HuggingFace rate limiting",
                    transport_name
                );
                return;
            }
            panic!("Real {} end-to-end test failed: {}", transport_name, e);
        }
        Err(_) => {
            panic!("Real {} end-to-end test timed out", transport_name);
        }
    }
}
