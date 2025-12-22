//! Conformance tests for ACP prompt turn protocol
//!
//! These tests verify that agent implementations correctly implement the ACP
//! prompt turn protocol per https://agentclientprotocol.com/protocol/prompt-turn
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

fn agent_agent_factory() -> std::pin::Pin<
    Box<dyn std::future::Future<Output = agent_fixtures::Result<Box<dyn Agent>>> + Send>,
> {
    Box::pin(async {
        let agent = agent_fixtures::create_agent().await?;
        Ok(Box::new(agent) as Box<dyn Agent>)
    })
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_basic_prompt_response(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::prompt_turn::test_basic_prompt_response(&*agent)
                .await
                .expect("Basic prompt response should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_prompt_completion(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::prompt_turn::test_prompt_completion(&*agent)
                .await
                .expect("Prompt completion test should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_stop_reasons(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::prompt_turn::test_stop_reasons(&*agent)
                .await
                .expect("Stop reasons test should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_cancellation(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::prompt_turn::test_cancellation(&*agent)
                .await
                .expect("Cancellation test should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_multiple_prompts(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::prompt_turn::test_multiple_prompts(&*agent)
                .await
                .expect("Multiple prompts test should succeed");
        })
        .await;
}
