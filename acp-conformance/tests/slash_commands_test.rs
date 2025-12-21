//! Conformance tests for ACP slash commands protocol
//!
//! These tests verify that agent implementations correctly implement the ACP
//! slash commands protocol per https://agentclientprotocol.com/protocol/slash-commands
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

// Agent factory functions
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
async fn test_command_structure_validation(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::slash_commands::test_command_structure_validation(&*agent)
                .await
                .expect("Command structure validation should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_advertise_commands(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::slash_commands::test_advertise_commands(&*agent)
                .await
                .expect("Command advertisement should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_run_command(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::slash_commands::test_run_command(&*agent)
                .await
                .expect("Running command should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_field_validation(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::slash_commands::test_command_field_validation(&*agent)
                .await
                .expect("Command field validation should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_input_hint(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::slash_commands::test_command_input_hint(&*agent)
                .await
                .expect("Command input hint validation should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_with_input(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::slash_commands::test_command_with_input(&*agent)
                .await
                .expect("Command with input should succeed");
        })
        .await;
}

#[rstest]
#[case::llama_agent(llama_agent_factory)]
#[case::claude_agent(claude_agent_factory)]
#[case::agent(agent_agent_factory)]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_with_mixed_content(#[case] factory: AgentFactory) {
    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async {
            let agent = factory().await.expect("Failed to create agent");
            acp_conformance::slash_commands::test_command_with_mixed_content(&*agent)
                .await
                .expect("Command with mixed content should succeed");
        })
        .await;
}
