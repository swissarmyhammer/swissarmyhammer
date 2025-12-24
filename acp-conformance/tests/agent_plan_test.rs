//! Conformance tests for ACP agent plan protocol

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
async fn test_agent_accepts_planning_prompt(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::agent_plan::test_agent_accepts_planning_prompt(&*agent)
        .await
        .expect("Agent accepts planning prompt should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_plan_entry_structure_validation(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::agent_plan::test_plan_entry_structure_validation(&*agent)
        .await
        .expect("Plan entry structure validation should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_plan_session_update_structure(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::agent_plan::test_plan_session_update_structure(&*agent)
        .await
        .expect("Plan session update structure should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_dynamic_plan_evolution(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::agent_plan::test_dynamic_plan_evolution(&*agent)
        .await
        .expect("Dynamic plan evolution should succeed");
}
