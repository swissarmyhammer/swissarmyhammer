//! Conformance tests for ACP prompt turn protocol

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
async fn test_basic_prompt_response(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::prompt_turn::test_basic_prompt_response(&*agent)
        .await
        .expect("Basic prompt response should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_prompt_completion(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::prompt_turn::test_prompt_completion(&*agent)
        .await
        .expect("Prompt completion test should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_stop_reasons(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::prompt_turn::test_stop_reasons(&*agent)
        .await
        .expect("Stop reasons test should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_cancellation(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::prompt_turn::test_cancellation(&*agent)
        .await
        .expect("Cancellation test should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_multiple_prompts(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::prompt_turn::test_multiple_prompts(&*agent)
        .await
        .expect("Multiple prompts test should succeed");
}
