//! Conformance tests for ACP content protocol

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
async fn test_text_content_support(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::content::test_text_content_support(&*agent)
        .await
        .expect("test_text_content_support should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_image_content_with_capability(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::content::test_image_content_with_capability(&*agent)
        .await
        .expect("test_image_content_with_capability should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_audio_content_with_capability(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::content::test_audio_content_with_capability(&*agent)
        .await
        .expect("test_audio_content_with_capability should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_embedded_resource_with_capability(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::content::test_embedded_resource_with_capability(&*agent)
        .await
        .expect("test_embedded_resource_with_capability should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_resource_link_content(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::content::test_resource_link_content(&*agent)
        .await
        .expect("test_resource_link_content should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_content_validation(
    #[case]
    #[future]
agent: Box<dyn AgentWithFixture>,
) {

    acp_conformance::content::test_content_validation(&*agent)
        .await
        .expect("test_content_validation should succeed");
}
