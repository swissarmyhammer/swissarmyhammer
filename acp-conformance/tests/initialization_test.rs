//! Conformance tests for ACP initialization protocol
//!
//! These tests verify that agent implementations correctly implement the ACP
//! initialization protocol per https://agentclientprotocol.com/protocol/initialization
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

// Agent factory functions that can be passed to rstest
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
async fn test_minimal_initialization(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::initialization::test_minimal_initialization(&*agent)
                .await
                .expect("Minimal initialization should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_full_capabilities_initialization(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::initialization::test_full_capabilities_initialization(&*agent)
                .await
                .expect("Full capabilities initialization should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_protocol_version_negotiation(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::initialization::test_protocol_version_negotiation(&*agent)
                .await
                .expect("Protocol version negotiation should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_minimal_client_capabilities(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::initialization::test_minimal_client_capabilities(&*agent)
                .await
                .expect("Minimal client capabilities should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_initialize_idempotent(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::initialization::test_initialize_idempotent(&*agent)
                .await
                .expect("Initialize idempotency test should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_with_client_info(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::initialization::test_with_client_info(&*agent)
                .await
                .expect("Client info test should succeed");
        })
        .await;
}
