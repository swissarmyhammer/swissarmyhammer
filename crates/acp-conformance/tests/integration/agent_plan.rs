//! Conformance tests for ACP agent plan protocol

use crate::common;

use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_agent_sends_plan_notifications(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::agent_plan::test_agent_sends_plan_notifications(&*agent)
        .await
        .expect("Agent sends plan notifications test should succeed");

    // Drop agent to trigger recording
    drop(agent);

    // Wait for recording to flush
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Verify fixture was created with expected structure
    acp_conformance::agent_plan::verify_plan_fixture(
        agent_type,
        "test_agent_sends_plan_notifications",
    )
    .expect("Fixture verification should succeed");
}

// Note: The previous validation-only tests (test_plan_entry_structure_validation,
// test_plan_session_update_structure, test_dynamic_plan_evolution) have been
// converted to unit tests in agent_plan.rs. They don't need agent fixtures
// since they only validate static JSON structures.
