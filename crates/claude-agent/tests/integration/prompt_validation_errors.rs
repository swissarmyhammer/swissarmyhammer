//! Every `validate_prompt_request` rejection says what was wrong.
//!
//! A rejection is the only thing the caller gets back: the turn never runs and
//! nothing is streamed. A bare `Error::invalid_params()` carries the code
//! `-32602`, the fixed message `"Invalid params"`, and `data: None`, so a
//! caller sees `failed to execute prompt: Invalid params` and cannot tell an
//! over-long prompt from an empty one from an unsupported content block.
//!
//! That happened in production: a review run rejected 36 fan-out tasks with
//! that one message and four separate agents each had to diagnose it from
//! scratch. The prompt was 14.9 MB against a 5 MB cap, and nothing in the
//! error said so.
//!
//! So each rejection names its own cause, and the over-length one names both
//! the actual length and the limit it broke.

use agent_client_protocol::schema::{
    ContentBlock, NewSessionRequest, PromptRequest, SessionId, SessionNotification, TextContent,
};
use agent_client_protocol::ErrorCode;
use claude_agent::{
    config::AgentConfig, constants::sizes::messages::MAX_PROMPT_LENGTH, ClaudeAgent,
};
use tokio::sync::broadcast;

/// Build an agent that never spawns the claude CLI.
///
/// No rejection these tests pin ever reaches the model, so no test here needs
/// a live CLI — and must not depend on one being installed.
async fn headless_agent() -> (ClaudeAgent, broadcast::Receiver<SessionNotification>) {
    let config = AgentConfig {
        spawn_claude_on_new_session: false,
        ..Default::default()
    };
    ClaudeAgent::new(config)
        .await
        .expect("agent construction should succeed")
}

/// Create a live session on `agent` and return its canonical id.
async fn new_session(agent: &ClaudeAgent) -> SessionId {
    agent
        .new_session(NewSessionRequest::new(
            std::env::current_dir().expect("a working directory"),
        ))
        .await
        .expect("session creation should succeed")
        .session_id
}

#[tokio::test]
async fn an_over_length_prompt_names_its_length_and_the_limit() {
    let (agent, _notifications) = headless_agent().await;
    let session = new_session(&agent).await;

    let over_by_one = MAX_PROMPT_LENGTH + 1;
    let rejected = agent
        .prompt(PromptRequest::new(
            session,
            vec![ContentBlock::Text(TextContent::new(
                "x".repeat(over_by_one),
            ))],
        ))
        .await
        .expect_err("a prompt over the configured cap is rejected");

    assert_eq!(
        rejected.code,
        ErrorCode::InvalidParams,
        "an over-length prompt stays an invalid-params rejection: {rejected:?}"
    );
    let message = rejected.message.to_string();
    assert!(
        message.contains(&over_by_one.to_string()),
        "the rejection names the prompt's actual length ({over_by_one}): {message}"
    );
    assert!(
        message.contains(&MAX_PROMPT_LENGTH.to_string()),
        "the rejection names the limit ({MAX_PROMPT_LENGTH}): {message}"
    );
}

#[tokio::test]
async fn a_prompt_with_no_content_says_so() {
    let (agent, _notifications) = headless_agent().await;
    let session = new_session(&agent).await;

    let rejected = agent
        .prompt(PromptRequest::new(session, vec![]))
        .await
        .expect_err("a prompt with no content is rejected");

    assert_eq!(rejected.code, ErrorCode::InvalidParams, "{rejected:?}");
    let message = rejected.message.to_string();
    assert!(
        message.contains("no content"),
        "the rejection names the empty prompt as the cause: {message}"
    );
    assert_ne!(
        message, "Invalid params",
        "a rejection must never be the bare code-only message"
    );
}
