//! Conformance tests for ACP content protocol

use crate::common;

use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_text_content_support(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::content::test_text_content_support(&*agent)
        .await
        .expect("test_text_content_support should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::content::verify_content_fixture_with_prompts(
        agent_type,
        "test_text_content_support",
        1,
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_image_content_with_capability(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::content::test_image_content_with_capability(&*agent)
        .await
        .expect("test_image_content_with_capability should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Image test may skip if no capability, so just verify basic fixture structure
    acp_conformance::content::verify_content_fixture(
        agent_type,
        "test_image_content_with_capability",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_audio_content_with_capability(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::content::test_audio_content_with_capability(&*agent)
        .await
        .expect("test_audio_content_with_capability should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Audio test may skip if no capability, so just verify basic fixture structure
    acp_conformance::content::verify_content_fixture(
        agent_type,
        "test_audio_content_with_capability",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_embedded_resource_with_capability(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::content::test_embedded_resource_with_capability(&*agent)
        .await
        .expect("test_embedded_resource_with_capability should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Embedded resource test may skip if no capability
    acp_conformance::content::verify_content_fixture(
        agent_type,
        "test_embedded_resource_with_capability",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_resource_link_content(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::content::test_resource_link_content(&*agent)
        .await
        .expect("test_resource_link_content should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::content::verify_content_fixture_with_prompts(
        agent_type,
        "test_resource_link_content",
        1,
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_content_validation(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::content::test_content_validation(&*agent)
        .await
        .expect("test_content_validation should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Content validation test sends prompts (may not all be recorded if errors occur)
    acp_conformance::content::verify_content_fixture(agent_type, "test_content_validation")
        .expect("Fixture verification should succeed");
}
