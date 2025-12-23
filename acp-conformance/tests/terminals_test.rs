//! Conformance tests for ACP terminals protocol

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
async fn test_terminal_capability_check(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_terminal_capability_check");
    acp_conformance::terminals::test_terminal_capability_check(&*agent)
        .await
        .expect("test_terminal_capability_check should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_create(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_terminal_create");
    acp_conformance::terminals::test_terminal_create(&*agent)
        .await
        .expect("test_terminal_create should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_output(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_terminal_output");
    acp_conformance::terminals::test_terminal_output(&*agent)
        .await
        .expect("test_terminal_output should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_wait_for_exit(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_terminal_wait_for_exit");
    acp_conformance::terminals::test_terminal_wait_for_exit(&*agent)
        .await
        .expect("test_terminal_wait_for_exit should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_kill(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_terminal_kill");
    acp_conformance::terminals::test_terminal_kill(&*agent)
        .await
        .expect("test_terminal_kill should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_release(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_terminal_release");
    acp_conformance::terminals::test_terminal_release(&*agent)
        .await
        .expect("test_terminal_release should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_terminal_timeout(#[case] #[future] mut agent: Box<dyn AgentWithFixture>) {
    agent.with_fixture("test_terminal_timeout");
    acp_conformance::terminals::test_terminal_timeout(&*agent)
        .await
        .expect("test_terminal_timeout should succeed");
}

