//! Conformance tests for ACP session setup and modes protocols

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
async fn test_new_session_minimal(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_new_session_minimal(&*agent)
        .await
        .expect("New session minimal should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_new_session_with_mcp(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_new_session_with_mcp(&*agent)
        .await
        .expect("New session with MCP should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_session_ids_unique(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_session_ids_unique(&*agent)
        .await
        .expect("Session IDs should be unique");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_load_nonexistent_session(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_load_nonexistent_session(&*agent)
        .await
        .expect("Load nonexistent session should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_session_mode(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_set_session_mode(&*agent)
        .await
        .expect("Set session mode should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_new_session_includes_modes(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_new_session_includes_modes(&*agent)
        .await
        .expect("New session includes modes should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_session_mode_to_available(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_set_session_mode_to_available(&*agent)
        .await
        .expect("Set session mode to available should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_invalid_session_mode(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_set_invalid_session_mode(&*agent)
        .await
        .expect("Set invalid session mode should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_mode_state_validation(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_mode_state_validation(&*agent)
        .await
        .expect("Mode state validation should succeed");
}

#[rstest]
#[case::llama(agent_fixtures::llama_agent_factory())]
#[case::claude(agent_fixtures::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_session_mode_independence(#[case] #[future] agent: Box<dyn AgentWithFixture>) {
    acp_conformance::sessions::test_session_mode_independence(&*agent)
        .await
        .expect("Session mode independence should succeed");
}
