//! Conformance tests for ACP initialization protocol

use crate::common;

use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_minimal_initialization(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::initialization::test_minimal_initialization(&*agent)
        .await
        .expect("Minimal initialization should succeed");

    // Drop agent to trigger recording
    drop(agent);

    // Wait for recording to flush
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Verify fixture was created with expected structure
    acp_conformance::initialization::verify_initialization_fixture(
        agent_type,
        "test_minimal_initialization",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_full_capabilities_initialization(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::initialization::test_full_capabilities_initialization(&*agent)
        .await
        .expect("Full capabilities initialization should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::initialization::verify_initialization_fixture(
        agent_type,
        "test_full_capabilities_initialization",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_protocol_version_negotiation(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::initialization::test_protocol_version_negotiation(&*agent)
        .await
        .expect("Protocol version negotiation should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::initialization::verify_initialization_fixture(
        agent_type,
        "test_protocol_version_negotiation",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_minimal_client_capabilities(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::initialization::test_minimal_client_capabilities(&*agent)
        .await
        .expect("Minimal client capabilities should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::initialization::verify_initialization_fixture(
        agent_type,
        "test_minimal_client_capabilities",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_initialize_idempotent(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::initialization::test_initialize_idempotent(&*agent)
        .await
        .expect("Initialize idempotent should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Idempotent test calls initialize twice
    let stats = acp_conformance::initialization::verify_initialization_fixture(
        agent_type,
        "test_initialize_idempotent",
    )
    .expect("Fixture verification should succeed");

    // Should have 2 initialize calls
    assert!(
        stats.initialize_calls >= 2,
        "Idempotent test should have at least 2 initialize calls, got {}",
        stats.initialize_calls
    );
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_with_client_info(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::initialization::test_with_client_info(&*agent)
        .await
        .expect("With client info should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::initialization::verify_initialization_fixture(
        agent_type,
        "test_with_client_info",
    )
    .expect("Fixture verification should succeed");
}
