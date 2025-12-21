//! Conformance tests for ACP file system protocol
//!
//! These tests verify that agent implementations correctly implement the ACP
//! file system protocol per https://agentclientprotocol.com/protocol/file-system
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
async fn test_read_text_file_capability_check(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::file_system::test_read_text_file_capability_check(&*agent)
                .await
                .expect("Read capability check should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_capability_check(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::file_system::test_write_text_file_capability_check(&*agent)
                .await
                .expect("Write capability check should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_text_file_basic(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::file_system::test_read_text_file_basic(&*agent)
                .await
                .expect("Basic read should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_text_file_with_range(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::file_system::test_read_text_file_with_range(&*agent)
                .await
                .expect("Read with range should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_basic(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::file_system::test_write_text_file_basic(&*agent)
                .await
                .expect("Basic write should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_creates_new(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::file_system::test_write_text_file_creates_new(&*agent)
                .await
                .expect("Write should create new file");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_write_integration(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::file_system::test_read_write_integration(&*agent)
                .await
                .expect("Read/write integration should succeed");
        })
        .await;
}
