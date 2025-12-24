//! Agent fixtures for conformance testing
//!
//! One factory per agent type. All return Box<dyn AgentWithFixture>.
//! Tests call agent.use_fixture("test_name") to auto-configure record/playback.

use agent_client_protocol_extras::AgentWithFixture;

/// Result type for agent creation
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Agent factory type for rstest
pub(crate) type AgentFactory = fn() -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<Box<dyn AgentWithFixture>>> + Send>,
>;

/// Llama agent factory for rstest
pub(crate) fn llama_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async {
        create_llama_agent()
            .await
            .expect("Failed to create llama agent")
    })
}

/// Claude agent factory for rstest
pub(crate) fn claude_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async {
        create_claude_agent()
            .await
            .expect("Failed to create claude agent")
    })
}

/// Generic agent factory for rstest (uses llama)
pub(crate) fn agent_agent_factory(
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Box<dyn AgentWithFixture>> + Send>> {
    Box::pin(async { create_agent().await.expect("Failed to create agent") })
}

/// Create claude-agent for testing
async fn create_claude_agent() -> Result<Box<dyn AgentWithFixture>> {
    use agent_client_protocol_extras::get_test_name_from_thread;

    let config = claude_agent::config::AgentConfig::default();
    let (agent, _receiver) = claude_agent::agent::ClaudeAgent::new(config).await?;

    let test_name = get_test_name_from_thread();
    Ok(agent_client_protocol_extras::with_fixture(agent, &test_name))
}

/// Create llama-agent for testing
async fn create_llama_agent() -> Result<Box<dyn AgentWithFixture>> {
    use agent_client_protocol_extras::get_test_name_from_thread;

    // Use test model config
    use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};
    let mut config = llama_agent::types::AgentConfig::default();
    config.model.source = llama_agent::types::ModelSource::HuggingFace {
        repo: TEST_MODEL_REPO.to_string(),
        filename: Some(TEST_MODEL_FILE.to_string()),
        folder: None,
    };

    let agent = llama_agent::acp::test_utils::create_acp_server(config).await?;

    let test_name = get_test_name_from_thread();
    Ok(agent_client_protocol_extras::with_fixture(agent, &test_name))
}

/// Create generic agent (uses llama)
async fn create_agent() -> Result<Box<dyn AgentWithFixture>> {
    create_llama_agent().await
}
