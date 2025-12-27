//! Conformance tests for ACP tool calls protocol

mod agent_fixtures;

use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_tool_call_notifications(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::tool_calls::test_tool_call_notifications(&*agent)
        .await
        .expect("Tool call notifications test should succeed");

    // Drop agent to trigger recording
    drop(agent);

    // Wait for recording to flush
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Verify fixture was created with expected notifications
    acp_conformance::tool_calls::verify_tool_call_fixture(
        agent_type,
        "test_tool_call_notifications",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_commands_update_notification(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::tool_calls::test_commands_update_notification(&*agent)
        .await
        .expect("Commands update notification test should succeed");
}
