//! Conformance tests for ACP terminal protocol
//!
//! These tests verify that agent implementations correctly implement the ACP
//! terminal protocol per https://agentclientprotocol.com/protocol/terminals
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
async fn test_terminal_capability_check(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::terminals::test_terminal_capability_check(&*agent)
                .await
                .expect("Terminal capability check should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_create(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::terminals::test_terminal_create(&*agent)
                .await
                .expect("Terminal create should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_output(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::terminals::test_terminal_output(&*agent)
                .await
                .expect("Terminal output should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_wait_for_exit(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::terminals::test_terminal_wait_for_exit(&*agent)
                .await
                .expect("Terminal wait for exit should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_kill(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::terminals::test_terminal_kill(&*agent)
                .await
                .expect("Terminal kill should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_release(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::terminals::test_terminal_release(&*agent)
                .await
                .expect("Terminal release should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_timeout(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::terminals::test_terminal_timeout(&*agent)
                .await
                .expect("Terminal timeout pattern should succeed");
        })
        .await;
}
