//! Conformance tests for ACP session setup and session modes protocols
//!
//! These tests verify that agent implementations correctly implement the ACP
//! session setup and session modes protocols per:
//! - https://agentclientprotocol.com/protocol/session-setup
//! - https://agentclientprotocol.com/protocol/session-modes
//!
//! Tests are parametrized using rstest to run against multiple agent implementations.
//! To add a new agent to test, simply add its factory function to the #[rstest] attributes.

mod agent_fixtures;

use agent_client_protocol::Agent;
use rstest::rstest;

// Helper type to make agent creation testable
type AgentFactory = fn() -> std::pin::Pin<
    Box<dyn std::future::Future<Output = agent_fixtures::Result<Box<dyn Agent>>> + Send>,
>;

// Agent factory function
fn llama_agent_factory() -> std::pin::Pin<
    Box<dyn std::future::Future<Output = agent_fixtures::Result<Box<dyn Agent>>> + Send>,
> {
    Box::pin(async {
        let agent = agent_fixtures::create_llama_agent().await?;
        Ok(Box::new(agent) as Box<dyn Agent>)
    })
}

fn claude_agent_factory() -> std::pin::Pin<
    Box<dyn std::future::Future<Output = agent_fixtures::Result<Box<dyn Agent>>> + Send>,
> {
    Box::pin(async {
        let agent = agent_fixtures::create_claude_agent().await?;
        Ok(Box::new(agent) as Box<dyn Agent>)
    })
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_new_session_minimal(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_new_session_minimal(&*agent)
                .await
                .expect("New session minimal should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_new_session_with_mcp(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_new_session_with_mcp(&*agent)
                .await
                .expect("New session with MCP should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_session_ids_unique(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_session_ids_unique(&*agent)
                .await
                .expect("Session IDs should be unique");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_load_nonexistent_session(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_load_nonexistent_session(&*agent)
                .await
                .expect("Load nonexistent should fail correctly");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_session_mode(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_set_session_mode(&*agent)
                .await
                .expect("Set session mode should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_new_session_includes_modes(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_new_session_includes_modes(&*agent)
                .await
                .expect("New session should include modes");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_session_mode_to_available(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_set_session_mode_to_available(&*agent)
                .await
                .expect("Set session mode to available should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_invalid_session_mode(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_set_invalid_session_mode(&*agent)
                .await
                .expect("Setting invalid mode should be rejected");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_mode_state_validation(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_mode_state_validation(&*agent)
                .await
                .expect("Mode state validation should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_session_mode_independence(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::sessions::test_session_mode_independence(&*agent)
                .await
                .expect("Session mode independence should succeed");
        })
        .await;
}
