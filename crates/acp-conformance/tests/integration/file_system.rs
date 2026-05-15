//! Conformance tests for ACP file system protocol

use crate::common;

use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_text_file_capability_check(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::file_system::test_read_text_file_capability_check(&*agent)
        .await
        .expect("test_read_text_file_capability_check should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::file_system::verify_file_system_fixture(
        agent_type,
        "test_read_text_file_capability_check",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_capability_check(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::file_system::test_write_text_file_capability_check(&*agent)
        .await
        .expect("test_write_text_file_capability_check should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::file_system::verify_file_system_fixture(
        agent_type,
        "test_write_text_file_capability_check",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_text_file_basic(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::file_system::test_read_text_file_basic(&*agent)
        .await
        .expect("test_read_text_file_basic should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::file_system::verify_file_system_fixture(
        agent_type,
        "test_read_text_file_basic",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_text_file_with_range(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::file_system::test_read_text_file_with_range(&*agent)
        .await
        .expect("test_read_text_file_with_range should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::file_system::verify_file_system_fixture(
        agent_type,
        "test_read_text_file_with_range",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_basic(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::file_system::test_write_text_file_basic(&*agent)
        .await
        .expect("test_write_text_file_basic should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::file_system::verify_file_system_fixture(
        agent_type,
        "test_write_text_file_basic",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_write_text_file_creates_new(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::file_system::test_write_text_file_creates_new(&*agent)
        .await
        .expect("test_write_text_file_creates_new should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::file_system::verify_file_system_fixture(
        agent_type,
        "test_write_text_file_creates_new",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_read_write_integration(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::file_system::test_read_write_integration(&*agent)
        .await
        .expect("test_read_write_integration should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::file_system::verify_file_system_fixture(
        agent_type,
        "test_read_write_integration",
    )
    .expect("Fixture verification should succeed");
}
