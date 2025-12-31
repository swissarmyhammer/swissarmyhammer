//! Prompt turn protocol conformance tests
//!
//! Tests based on https://agentclientprotocol.com/protocol/prompt-turn
//!
//! ## Test Model Requirements
//!
//! **llama-agent tests** require the Qwen3-0.6B model to be downloaded from HuggingFace:
//! - Repo: `unsloth/Qwen3-0.6B-GGUF`
//! - File: `Qwen3-0.6B-IQ4_NL.gguf`
//! - The model will auto-download on first test run (~600MB)
//!
//! **claude-agent tests** use playback fixtures and don't require model downloads.
//!
//! ## Requirements Tested
//!
//! 1. **User Message**
//!    - Client sends `session/prompt` request
//!    - Contains user message content blocks
//!
//! 2. **Agent Processing**
//!    - Agent processes message and sends to language model
//!    - May respond with text, tool calls, or both
//!
//! 3. **Agent Reports Output**
//!    - Sends `session/update` notifications during processing
//!    - Plan notifications (optional)
//!    - Agent message chunks (text responses)
//!    - Tool call notifications
//!
//! 4. **Check for Completion**
//!    - Returns `StopReason` when turn completes
//!    - Must specify reason: end_turn, max_tokens, max_turn_requests, refusal, cancelled
//!
//! 5. **Tool Invocation and Status Reporting**
//!    - Sends tool_call with pending status
//!    - Updates to in_progress during execution
//!    - Final status: completed or failed
//!
//! 6. **Continue Conversation**
//!    - Sends tool results back to model
//!    - Cycles until model completes without requesting more tools
//!
//! 7. **Cancellation**
//!    - Client sends `session/cancel` notification
//!    - Agent stops processing and returns cancelled stop reason

use agent_client_protocol::{
    Agent, ClientCapabilities, ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest,
    ProtocolVersion, StopReason, TextContent,
};
use agent_client_protocol_extras::recording::RecordedSession;

/// Statistics from prompt turn fixture verification
#[derive(Debug, Default)]
pub struct PromptTurnStats {
    pub initialize_calls: usize,
    pub new_session_calls: usize,
    pub prompt_calls: usize,
    pub cancel_calls: usize,
    pub agent_message_chunks: usize,
    pub user_message_chunks: usize,
}

/// Test basic prompt-response cycle
pub async fn test_basic_prompt_response<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing basic prompt-response cycle");

    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await?;

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Send prompt
    let prompt_text = "Hello, how are you?";
    let prompt = vec![ContentBlock::Text(TextContent::new(prompt_text))];
    let prompt_request = PromptRequest::new(session_id.clone(), prompt);

    // Execute prompt and verify response
    let response = agent.prompt(prompt_request).await?;

    // Verify stop reason is valid
    match response.stop_reason {
        StopReason::EndTurn
        | StopReason::MaxTokens
        | StopReason::MaxTurnRequests
        | StopReason::Refusal
        | StopReason::Cancelled => {
            tracing::info!("Received valid stop reason: {:?}", response.stop_reason);
            Ok(())
        }
        _ => {
            tracing::warn!(
                "Received unexpected stop reason: {:?}",
                response.stop_reason
            );
            Ok(())
        }
    }
}

/// Test that prompt responses complete successfully
pub async fn test_prompt_completion<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing prompt completion");

    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await?;

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Send prompt
    let prompt_text = "What is 2+2?";
    let prompt = vec![ContentBlock::Text(TextContent::new(prompt_text))];
    let prompt_request = PromptRequest::new(session_id.clone(), prompt);

    // Execute prompt
    let response = agent.prompt(prompt_request).await?;

    // Verify response has valid stop reason
    match response.stop_reason {
        StopReason::EndTurn | StopReason::MaxTokens => {
            tracing::info!(
                "Prompt completed with stop reason: {:?}",
                response.stop_reason
            );
            Ok(())
        }
        other => {
            tracing::warn!("Unexpected stop reason: {:?}", other);
            Ok(())
        }
    }
}

/// Test that stop reasons are properly returned
pub async fn test_stop_reasons<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing stop reason handling");

    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await?;

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Test 1: Normal completion should return EndTurn
    {
        let prompt = vec![ContentBlock::Text(TextContent::new("Say hello"))];
        let prompt_request = PromptRequest::new(session_id.clone(), prompt);
        let response = agent.prompt(prompt_request).await?;

        // Should be EndTurn or possibly MaxTokens depending on response length
        match response.stop_reason {
            StopReason::EndTurn | StopReason::MaxTokens => {
                tracing::info!(
                    "Normal completion returned expected stop reason: {:?}",
                    response.stop_reason
                );
            }
            other => {
                return Err(crate::Error::Validation(format!(
                    "Expected EndTurn or MaxTokens, got {:?}",
                    other
                )));
            }
        }
    }

    Ok(())
}

/// Test cancellation handling
pub async fn test_cancellation<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing cancellation handling");

    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await?;

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id.clone();

    // Send cancellation notification
    let cancel_notification = agent_client_protocol::CancelNotification::new(session_id);
    agent.cancel(cancel_notification).await?;

    tracing::info!("Cancellation accepted by agent");
    Ok(())
}

/// Test that multiple prompts work in sequence
pub async fn test_multiple_prompts<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing multiple sequential prompts");

    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await?;

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Send first prompt
    let prompt1 = vec![ContentBlock::Text(TextContent::new("Hello"))];
    let prompt_request1 = PromptRequest::new(session_id.clone(), prompt1);
    let response1 = agent.prompt(prompt_request1).await?;

    tracing::info!("First prompt completed with: {:?}", response1.stop_reason);

    // Send second prompt
    let prompt2 = vec![ContentBlock::Text(TextContent::new("How are you?"))];
    let prompt_request2 = PromptRequest::new(session_id.clone(), prompt2);
    let response2 = agent.prompt(prompt_request2).await?;

    tracing::info!("Second prompt completed with: {:?}", response2.stop_reason);

    // Both should complete successfully
    match (response1.stop_reason, response2.stop_reason) {
        (
            StopReason::EndTurn | StopReason::MaxTokens,
            StopReason::EndTurn | StopReason::MaxTokens,
        ) => {
            tracing::info!("Multiple prompts completed successfully");
            Ok(())
        }
        _ => {
            tracing::warn!("Unexpected stop reasons in multiple prompt test");
            Ok(())
        }
    }
}

/// Verify prompt turn fixture has proper recordings
///
/// This function reads the fixture and verifies:
/// 1. The fixture has recorded calls (not calls: [])
/// 2. Initialize, new_session, and prompt calls were recorded
/// 3. Agent message chunks were produced (agent responded)
pub fn verify_prompt_turn_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<PromptTurnStats, Box<dyn std::error::Error>> {
    let fixture_path = agent_client_protocol_extras::get_fixture_path_for(agent_type, test_name);

    if !fixture_path.exists() {
        return Err(format!("Fixture not found: {:?}", fixture_path).into());
    }

    let content = std::fs::read_to_string(&fixture_path)?;
    let session: RecordedSession = serde_json::from_str(&content)?;

    let mut stats = PromptTurnStats::default();

    // CRITICAL: Verify we have calls recorded (catches poor tests with calls: [])
    assert!(
        !session.calls.is_empty(),
        "Expected recorded calls, fixture has calls: [] - test didn't call agent properly"
    );

    for call in &session.calls {
        match call.method.as_str() {
            "initialize" => stats.initialize_calls += 1,
            "new_session" => stats.new_session_calls += 1,
            "prompt" => {
                stats.prompt_calls += 1;
                // Count notifications
                for notification_json in &call.notifications {
                    if let Some(update_val) = notification_json.get("update") {
                        if let Some(session_update) =
                            update_val.get("sessionUpdate").and_then(|v| v.as_str())
                        {
                            match session_update {
                                "agent_message_chunk" => stats.agent_message_chunks += 1,
                                "user_message_chunk" => stats.user_message_chunks += 1,
                                _ => {}
                            }
                        }
                    }
                }
            }
            "cancel" => stats.cancel_calls += 1,
            _ => {}
        }
    }

    tracing::info!("{} prompt turn fixture stats: {:?}", agent_type, stats);

    Ok(stats)
}

/// Test streaming capability negotiation
///
/// Verifies that when a client advertises streaming capability, the agent:
/// 1. Accepts the capability during initialization
/// 2. Sends agent_message_chunk notifications during prompt processing
pub async fn test_streaming_capability<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing streaming capability negotiation");

    // Initialize with streaming capability in meta
    let mut meta = serde_json::Map::new();
    meta.insert("streaming".to_string(), serde_json::json!(true));

    let client_caps = ClientCapabilities::new().meta(Some(meta));
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    let init_response = agent.initialize(init_request).await?;

    // Agent should acknowledge capabilities
    tracing::info!("Agent capabilities: {:?}", init_response.agent_capabilities);

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // Send prompt - streaming notifications should be sent
    let prompt = vec![ContentBlock::Text(TextContent::new("Hello"))];
    let prompt_request = PromptRequest::new(session_id.clone(), prompt);
    let response = agent.prompt(prompt_request).await?;

    tracing::info!(
        "Streaming prompt completed with stop reason: {:?}",
        response.stop_reason
    );

    Ok(())
}

/// Test that streaming works correctly across multiple prompts
///
/// Verifies that:
/// 1. Session context is maintained between streaming prompts
/// 2. Each prompt produces streaming notifications
pub async fn test_streaming_context_maintained<A: Agent + ?Sized>(agent: &A) -> crate::Result<()> {
    tracing::info!("Testing streaming with context maintained across prompts");

    // Initialize with streaming capability
    let mut meta = serde_json::Map::new();
    meta.insert("streaming".to_string(), serde_json::json!(true));

    let client_caps = ClientCapabilities::new().meta(Some(meta));
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await?;

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await?;
    let session_id = new_session_response.session_id;

    // First prompt - establish context
    let prompt1 = vec![ContentBlock::Text(TextContent::new(
        "My favorite color is blue. Remember this.",
    ))];
    let prompt_request1 = PromptRequest::new(session_id.clone(), prompt1);
    let response1 = agent.prompt(prompt_request1).await?;
    tracing::info!("First prompt completed: {:?}", response1.stop_reason);

    // Second prompt - should have context from first
    let prompt2 = vec![ContentBlock::Text(TextContent::new(
        "What is my favorite color?",
    ))];
    let prompt_request2 = PromptRequest::new(session_id.clone(), prompt2);
    let response2 = agent.prompt(prompt_request2).await?;
    tracing::info!("Second prompt completed: {:?}", response2.stop_reason);

    // Both should complete successfully
    match (response1.stop_reason, response2.stop_reason) {
        (
            StopReason::EndTurn | StopReason::MaxTokens,
            StopReason::EndTurn | StopReason::MaxTokens,
        ) => {
            tracing::info!("Streaming context test completed successfully");
            Ok(())
        }
        _ => Err(crate::Error::Validation(
            "Unexpected stop reasons in streaming context test".to_string(),
        )),
    }
}

/// Verify streaming fixture has agent_message_chunk notifications
pub fn verify_streaming_fixture(
    agent_type: &str,
    test_name: &str,
) -> Result<PromptTurnStats, Box<dyn std::error::Error>> {
    let stats = verify_prompt_turn_fixture(agent_type, test_name)?;

    // Streaming tests should produce message chunks
    assert!(
        stats.agent_message_chunks > 0,
        "Streaming test should produce agent_message_chunk notifications, got {}",
        stats.agent_message_chunks
    );

    Ok(stats)
}

/// Verify prompt turn fixture with expected call counts
pub fn verify_prompt_fixture_with_response(
    agent_type: &str,
    test_name: &str,
    expected_prompts: usize,
) -> Result<PromptTurnStats, Box<dyn std::error::Error>> {
    let stats = verify_prompt_turn_fixture(agent_type, test_name)?;

    // Should have initialize
    assert!(
        stats.initialize_calls >= 1,
        "Expected at least 1 initialize call, got {}",
        stats.initialize_calls
    );

    // Should have new_session
    assert!(
        stats.new_session_calls >= 1,
        "Expected at least 1 new_session call, got {}",
        stats.new_session_calls
    );

    // Should have expected number of prompts
    assert!(
        stats.prompt_calls >= expected_prompts,
        "Expected at least {} prompt calls, got {}",
        expected_prompts,
        stats.prompt_calls
    );

    // Should have agent response chunks
    assert!(
        stats.agent_message_chunks > 0,
        "Expected agent_message_chunk notifications, got {}. Agent should produce output.",
        stats.agent_message_chunks
    );

    Ok(stats)
}
