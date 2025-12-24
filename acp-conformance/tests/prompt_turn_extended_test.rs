//! Extended integration tests for prompt turn protocol conformance
//!
//! These tests complement the basic prompt_turn tests with additional edge cases,
//! error handling scenarios, and complex interaction patterns.

mod agent_fixtures;
mod common;

use agent_client_protocol::{
    Agent, ClientCapabilities, ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest,
    ProtocolVersion, StopReason, TextContent,
};
use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

/// Test handling of empty prompt content
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_empty_prompt_handling(
    #[case] #[future]agent: Box<dyn AgentWithFixture>,
) {


    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await.unwrap();

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = new_session_response.session_id;

    // Send empty prompt - should be handled gracefully
    let empty_prompt = vec![ContentBlock::Text(TextContent::new(""))];
    let prompt_request = PromptRequest::new(session_id, empty_prompt);

    // Agent should either accept it or return a meaningful error
    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(response) => {
            tracing::info!("Agent accepted empty prompt with: {:?}", response.stop_reason);
            assert!(matches!(
                response.stop_reason,
                StopReason::EndTurn
                    | StopReason::MaxTokens
                    | StopReason::MaxTurnRequests
                    | StopReason::Refusal
                    | StopReason::Cancelled
            ));
        }
        Err(_) => {
            tracing::info!("Agent rejected empty prompt (acceptable behavior)");
        }
    }
}

/// Test handling of very long prompts
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_long_prompt_handling(#[case] #[future]agent: Box<dyn AgentWithFixture>) {


    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await.unwrap();

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = new_session_response.session_id;

    // Create a very long prompt (1000 repetitions)
    let long_text = "This is a test sentence. ".repeat(1000);
    let prompt = vec![ContentBlock::Text(TextContent::new(long_text))];
    let prompt_request = PromptRequest::new(session_id, prompt);

    // Agent should handle long input gracefully
    let result = agent.prompt(prompt_request).await;

    match result {
        Ok(response) => {
            tracing::info!(
                "Agent processed long prompt with: {:?}",
                response.stop_reason
            );
            assert!(matches!(
                response.stop_reason,
                StopReason::EndTurn
                    | StopReason::MaxTokens
                    | StopReason::MaxTurnRequests
                    | StopReason::Refusal
            ));
        }
        Err(e) => {
            tracing::info!("Agent rejected long prompt: {:?} (acceptable if context limit exceeded)", e);
        }
    }
}

/// Test prompt with special characters and unicode
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_special_characters_in_prompt(
    #[case] #[future]agent: Box<dyn AgentWithFixture>,
) {


    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await.unwrap();

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = new_session_response.session_id;

    // Prompt with various special characters and unicode
    let special_text = "Hello! Test with émojis 🚀, symbols ©®™, and newlines:\n\nNew paragraph.";
    let prompt = vec![ContentBlock::Text(TextContent::new(special_text))];
    let prompt_request = PromptRequest::new(session_id, prompt);

    let response = agent.prompt(prompt_request).await.unwrap();

    tracing::info!(
        "Agent processed special characters with: {:?}",
        response.stop_reason
    );
    assert!(matches!(
        response.stop_reason,
        StopReason::EndTurn | StopReason::MaxTokens
    ));
}

/// Test multiple content blocks in a single prompt
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_multiple_content_blocks(#[case] #[future]agent: Box<dyn AgentWithFixture>) {


    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await.unwrap();

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = new_session_response.session_id;

    // Send prompt with multiple text blocks
    let prompt = vec![
        ContentBlock::Text(TextContent::new("First block: What is 2+2?")),
        ContentBlock::Text(TextContent::new("Second block: And what is 3+3?")),
        ContentBlock::Text(TextContent::new("Third block: Please answer both.")),
    ];
    let prompt_request = PromptRequest::new(session_id, prompt);

    let response = agent.prompt(prompt_request).await.unwrap();

    tracing::info!(
        "Agent processed multiple blocks with: {:?}",
        response.stop_reason
    );
    assert!(matches!(
        response.stop_reason,
        StopReason::EndTurn | StopReason::MaxTokens
    ));
}

/// Test rapid sequential prompts (stress test)
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_rapid_sequential_prompts(#[case] #[future]agent: Box<dyn AgentWithFixture>) {


    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await.unwrap();

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = new_session_response.session_id;

    // Send 5 rapid prompts
    for i in 1..=5 {
        let prompt_text = format!("Quick test #{}", i);
        let prompt = vec![ContentBlock::Text(TextContent::new(prompt_text))];
        let prompt_request = PromptRequest::new(session_id.clone(), prompt);

        let response = agent.prompt(prompt_request).await.unwrap();

        tracing::info!("Prompt {} completed with: {:?}", i, response.stop_reason);
        assert!(matches!(
            response.stop_reason,
            StopReason::EndTurn
                | StopReason::MaxTokens
                | StopReason::MaxTurnRequests
                | StopReason::Refusal
        ));
    }

    tracing::info!("All rapid sequential prompts completed successfully");
}

/// Test prompt in invalid session
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_prompt_invalid_session(#[case] #[future]agent: Box<dyn AgentWithFixture>) {


    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await.unwrap();

    // Use a non-existent session ID
    let invalid_session_id = agent_client_protocol::SessionId("invalid_session_xyz".to_string());

    // Send prompt to invalid session
    let prompt = vec![ContentBlock::Text(TextContent::new("Hello"))];
    let prompt_request = PromptRequest::new(invalid_session_id, prompt);

    let result = agent.prompt(prompt_request).await;

    // Should fail with appropriate error
    assert!(
        result.is_err(),
        "Agent should reject prompt to invalid session"
    );
    tracing::info!("Agent correctly rejected invalid session prompt");
}

/// Test stop reason end_turn is most common
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_end_turn_stop_reason_common(
    #[case] #[future]agent: Box<dyn AgentWithFixture>,
) {


    // Initialize agent
    let client_caps = ClientCapabilities::new();
    let init_request = InitializeRequest::new(ProtocolVersion::V1).client_capabilities(client_caps);
    agent.initialize(init_request).await.unwrap();

    // Create session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let new_session_response = agent.new_session(new_session_request).await.unwrap();
    let session_id = new_session_response.session_id;

    // Send simple prompt that should complete normally
    let prompt = vec![ContentBlock::Text(TextContent::new("Say hello in one word"))];
    let prompt_request = PromptRequest::new(session_id, prompt);

    let response = agent.prompt(prompt_request).await.unwrap();

    // Short responses should typically end with EndTurn
    tracing::info!("Stop reason received: {:?}", response.stop_reason);
    // We accept both EndTurn and MaxTokens as valid
    assert!(matches!(
        response.stop_reason,
        StopReason::EndTurn | StopReason::MaxTokens
    ));
}

/// Test that prompts without initialization fail appropriately
#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_prompt_without_initialization(
    #[case] #[future]agent: Box<dyn AgentWithFixture>,
) {


    // Create session WITHOUT initializing
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let new_session_request = NewSessionRequest::new(cwd);
    let result = agent.new_session(new_session_request).await;

    // Should fail or warn appropriately
    match result {
        Ok(_) => {
            tracing::warn!("Agent allowed session creation without initialization (lenient behavior)");
        }
        Err(e) => {
            tracing::info!("Agent correctly rejected session without initialization: {:?}", e);
        }
    }
}
