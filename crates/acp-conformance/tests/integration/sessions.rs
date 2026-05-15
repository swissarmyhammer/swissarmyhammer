//! Conformance tests for ACP session setup and modes protocols

use crate::common;

use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_new_session_minimal(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_new_session_minimal(&*agent)
        .await
        .expect("New session minimal should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::sessions::verify_new_session_fixture(
        agent_type,
        "test_new_session_minimal",
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
async fn test_new_session_with_mcp(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_new_session_with_mcp(&*agent)
        .await
        .expect("New session with MCP should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::sessions::verify_new_session_fixture(
        agent_type,
        "test_new_session_with_mcp",
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
async fn test_session_ids_unique(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_session_ids_unique(&*agent)
        .await
        .expect("Session IDs should be unique");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // This test creates 2 sessions
    acp_conformance::sessions::verify_new_session_fixture(agent_type, "test_session_ids_unique", 2)
        .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_load_nonexistent_session(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_load_nonexistent_session(&*agent)
        .await
        .expect("Load nonexistent session should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // verify_session_fixture checks for non-empty calls, which ensures we called initialize
    // Note: load_session errors may not always be recorded depending on agent impl
    acp_conformance::sessions::verify_session_fixture(agent_type, "test_load_nonexistent_session")
        .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_session_mode(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_set_session_mode(&*agent)
        .await
        .expect("Set session mode should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // This test creates a session, then possibly sets a mode
    acp_conformance::sessions::verify_session_fixture(agent_type, "test_set_session_mode")
        .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_new_session_includes_modes(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_new_session_includes_modes(&*agent)
        .await
        .expect("New session includes modes should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::sessions::verify_new_session_fixture(
        agent_type,
        "test_new_session_includes_modes",
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
async fn test_set_session_mode_to_available(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_set_session_mode_to_available(&*agent)
        .await
        .expect("Set session mode to available should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::sessions::verify_session_fixture(
        agent_type,
        "test_set_session_mode_to_available",
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
// llama agent does not validate mode IDs, so it accepts any mode without error
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_set_invalid_session_mode(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_set_invalid_session_mode(&*agent)
        .await
        .expect("Set invalid session mode should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::sessions::verify_session_fixture(agent_type, "test_set_invalid_session_mode")
        .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_mode_state_validation(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_mode_state_validation(&*agent)
        .await
        .expect("Mode state validation should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::sessions::verify_new_session_fixture(
        agent_type,
        "test_mode_state_validation",
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
async fn test_session_mode_independence(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::sessions::test_session_mode_independence(&*agent)
        .await
        .expect("Session mode independence should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // This test creates 2 sessions
    acp_conformance::sessions::verify_new_session_fixture(
        agent_type,
        "test_session_mode_independence",
        2,
    )
    .expect("Fixture verification should succeed");
}
