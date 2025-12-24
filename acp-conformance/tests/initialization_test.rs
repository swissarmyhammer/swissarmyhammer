//! Conformance tests for ACP initialization protocol

mod agent_fixtures;

use agent_client_protocol::Agent;
use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_minimal_initialization(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::initialization::test_minimal_initialization(&*agent)
        .await
        .expect("Minimal initialization should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_full_capabilities_initialization(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::initialization::test_full_capabilities_initialization(&*agent)
        .await
        .expect("Full capabilities initialization should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_protocol_version_negotiation(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::initialization::test_protocol_version_negotiation(&*agent)
        .await
        .expect("Protocol version negotiation should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_minimal_client_capabilities(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::initialization::test_minimal_client_capabilities(&*agent)
        .await
        .expect("Minimal client capabilities should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_initialize_idempotent(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::initialization::test_initialize_idempotent(&*agent)
        .await
        .expect("Initialize idempotent should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_with_client_info(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::initialization::test_with_client_info(&*agent)
        .await
        .expect("With client info should succeed");
}
