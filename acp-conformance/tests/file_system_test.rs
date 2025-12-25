//! Conformance tests for ACP file system protocol

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
async fn test_read_text_file_capability_check(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::file_system::test_read_text_file_capability_check(&*agent)
        .await
        .expect("test_read_text_file_capability_check should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_capability_check(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::file_system::test_write_text_file_capability_check(&*agent)
        .await
        .expect("test_write_text_file_capability_check should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_text_file_basic(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::file_system::test_read_text_file_basic(&*agent)
        .await
        .expect("test_read_text_file_basic should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_text_file_with_range(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::file_system::test_read_text_file_with_range(&*agent)
        .await
        .expect("test_read_text_file_with_range should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_basic(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::file_system::test_write_text_file_basic(&*agent)
        .await
        .expect("test_write_text_file_basic should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_creates_new(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::file_system::test_write_text_file_creates_new(&*agent)
        .await
        .expect("test_write_text_file_creates_new should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_write_integration(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    acp_conformance::file_system::test_read_write_integration(&*agent)
        .await
        .expect("test_read_write_integration should succeed");
}
