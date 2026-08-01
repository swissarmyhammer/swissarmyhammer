//! Every `session/prompt` exit path emits the end-of-turn marker.
//!
//! A client that reassembles a turn's reply from streamed chunks drains the
//! session's notification stream until the agent's end-of-turn marker arrives
//! (see [`claude_agent::collect_response_content`]). A `prompt` call that
//! returns WITHOUT emitting the marker leaves that client waiting for the
//! drain's hang guard — [`claude_agent::NOTIFICATION_DRAIN_BACKSTOP_MS`] — and
//! then failing, even though the turn is over and the caller already has its
//! answer.
//!
//! A rejected prompt is exactly that case: the session is live and already
//! subscribed, and the rejection ends the turn just as a reply does. These
//! tests pin the marker on that path.

use agent_client_protocol::schema::{NewSessionRequest, PromptRequest, PromptResponse, StopReason};
use agent_client_protocol::ErrorCode;
use claude_agent::{config::AgentConfig, ClaudeAgent};
use std::time::Duration;

/// How long the drain may take before the test calls it a hang.
///
/// Far below [`claude_agent::NOTIFICATION_DRAIN_BACKSTOP_MS`] (10 s): the drain
/// must end on the marker, never on the hang guard. Generous enough to absorb
/// scheduling latency on a loaded machine, small enough that hitting the
/// backstop cannot pass for success.
const DRAIN_MUST_FINISH_WITHIN: Duration = Duration::from_secs(2);

/// A prompt rejected by request validation still marks its turn complete, so a
/// collector that subscribed before the prompt stops immediately instead of
/// waiting out the drain's backstop.
///
/// An empty `prompt` array carries no content, which `validate_prompt_request`
/// rejects before any turn work begins, so no turn ever reaches the backend.
/// The claude CLI is not spawned at all: `spawn_claude_on_new_session` is off,
/// the headless seam this test shares with `mcp_http_session`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_rejected_prompt_ends_the_turn_for_a_subscribed_collector() {
    let config = AgentConfig {
        spawn_claude_on_new_session: false,
        ..Default::default()
    };
    let (agent, notifications) = ClaudeAgent::new(config)
        .await
        .expect("agent construction should succeed");

    let session = agent
        .new_session(NewSessionRequest::new(
            std::env::current_dir().expect("a working directory"),
        ))
        .await
        .expect("session creation should succeed")
        .session_id;

    // Subscribe exactly as a pool client does: before the prompt is sent.
    let collector =
        claude_agent::spawn_notification_collector(notifications.resubscribe(), session.clone());

    let rejected = agent
        .prompt(PromptRequest::new(session, vec![]))
        .await
        .expect_err("a prompt with no content is invalid");
    assert_eq!(
        rejected.code,
        ErrorCode::InvalidParams,
        "an empty prompt is rejected as invalid params: {rejected:?}"
    );

    let prompt_response = PromptResponse::new(StopReason::EndTurn);
    let content = tokio::time::timeout(
        DRAIN_MUST_FINISH_WITHIN,
        claude_agent::collect_response_content(collector, &prompt_response),
    )
    .await
    .expect("a rejected prompt must emit its end-of-turn marker, not leave the collector waiting for the backstop")
    .expect("the drain reached the end of the turn's stream");

    assert!(
        content.is_empty(),
        "a rejected prompt streams no reply: {content:?}"
    );
}
