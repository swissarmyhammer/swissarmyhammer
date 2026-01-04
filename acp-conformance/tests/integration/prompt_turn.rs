//! Conformance tests for ACP prompt turn protocol

use crate::common;

use agent_client_protocol_extras::AgentWithFixture;
use rstest::rstest;

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_basic_prompt_response(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::prompt_turn::test_basic_prompt_response(&*agent)
        .await
        .expect("Basic prompt response should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::prompt_turn::verify_prompt_fixture_with_response(
        agent_type,
        "test_basic_prompt_response",
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
async fn test_prompt_completion(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::prompt_turn::test_prompt_completion(&*agent)
        .await
        .expect("Prompt completion test should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::prompt_turn::verify_prompt_fixture_with_response(
        agent_type,
        "test_prompt_completion",
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
async fn test_stop_reasons(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::prompt_turn::test_stop_reasons(&*agent)
        .await
        .expect("Stop reasons test should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::prompt_turn::verify_prompt_fixture_with_response(
        agent_type,
        "test_stop_reasons",
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
async fn test_cancellation(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::prompt_turn::test_cancellation(&*agent)
        .await
        .expect("Cancellation test should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // Cancellation test doesn't send prompts, just creates session and cancels
    let stats =
        acp_conformance::prompt_turn::verify_prompt_turn_fixture(agent_type, "test_cancellation")
            .expect("Fixture verification should succeed");

    // Should have at least init and session creation
    assert!(stats.initialize_calls >= 1, "Expected initialize calls");
    assert!(stats.new_session_calls >= 1, "Expected new_session calls");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_multiple_prompts(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::prompt_turn::test_multiple_prompts(&*agent)
        .await
        .expect("Multiple prompts test should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // This test sends 2 prompts
    acp_conformance::prompt_turn::verify_prompt_fixture_with_response(
        agent_type,
        "test_multiple_prompts",
        2,
    )
    .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_streaming_capability(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::prompt_turn::test_streaming_capability(&*agent)
        .await
        .expect("Streaming capability test should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    acp_conformance::prompt_turn::verify_streaming_fixture(agent_type, "test_streaming_capability")
        .expect("Fixture verification should succeed");
}

#[rstest]
#[case::llama(common::llama_agent_factory())]
#[case::claude(common::claude_agent_factory())]
#[awt]
#[test_log::test(tokio::test)]
#[serial_test::serial]
async fn test_streaming_context_maintained(
    #[case]
    #[future]
    agent: Box<dyn AgentWithFixture>,
) {
    let agent_type = agent.agent_type();

    acp_conformance::prompt_turn::test_streaming_context_maintained(&*agent)
        .await
        .expect("Streaming context test should succeed");

    drop(agent);
    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    // This test sends 2 prompts with streaming
    acp_conformance::prompt_turn::verify_prompt_fixture_with_response(
        agent_type,
        "test_streaming_context_maintained",
        2,
    )
    .expect("Fixture verification should succeed");
}
