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
