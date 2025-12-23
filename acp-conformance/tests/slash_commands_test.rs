//! Conformance tests for ACP slash commands protocol

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
async fn test_command_structure_validation(
    #[case]
    #[future]
    mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_command_structure_validation");
    acp_conformance::slash_commands::test_command_structure_validation(&*agent)
        .await
        .expect("test_command_structure_validation should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_advertise_commands(
    #[case]
    #[future]
    mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_advertise_commands");
    acp_conformance::slash_commands::test_advertise_commands(&*agent)
        .await
        .expect("test_advertise_commands should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_run_command(
    #[case]
    #[future]
    mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_run_command");
    acp_conformance::slash_commands::test_run_command(&*agent)
        .await
        .expect("test_run_command should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_field_validation(
    #[case]
    #[future]
    mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_command_field_validation");
    acp_conformance::slash_commands::test_command_field_validation(&*agent)
        .await
        .expect("test_command_field_validation should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_input_hint(
    #[case]
    #[future]
    mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_command_input_hint");
    acp_conformance::slash_commands::test_command_input_hint(&*agent)
        .await
        .expect("test_command_input_hint should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_with_input(
    #[case]
    #[future]
    mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_command_with_input");
    acp_conformance::slash_commands::test_command_with_input(&*agent)
        .await
        .expect("test_command_with_input should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_command_with_mixed_content(
    #[case]
    #[future]
    mut agent: Box<dyn AgentWithFixture>,
) {
    agent.with_fixture("test_command_with_mixed_content");
    acp_conformance::slash_commands::test_command_with_mixed_content(&*agent)
        .await
        .expect("test_command_with_mixed_content should succeed");
}
