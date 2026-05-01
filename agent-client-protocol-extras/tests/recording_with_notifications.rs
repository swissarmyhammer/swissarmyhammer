//! End-to-end tests for [`agent_client_protocol_extras::RecordingAgent::with_notifications`]
//! and `add_mcp_source` — the multi-source notification capture used by the
//! ACP conformance fixture-recording flow.
//!
//! These tests stand up a complete pipeline:
//!
//! ```text
//!  ┌──────────────┐    duplex     ┌────────────────┐     duplex     ┌────────────────┐
//!  │ test client  │  <─────────>  │ RecordingAgent │  <─────────>   │ PlaybackAgent  │
//!  │  (Builder)   │   (recorded)  │  (middleware)  │  (re-emit)     │  (test stub)   │
//!  └──────────────┘               └───────┬────────┘                └────────────────┘
//!                                         │
//!                                  add_mcp_source
//!                                         │
//!                                 broadcast::Sender<McpNotification>
//! ```
//!
//! The PlaybackAgent serves a hand-crafted prompt response that includes a
//! recorded `session/update` notification. RecordingAgent observes this on
//! the wire and folds it into its in-memory recording. We then push a
//! synthetic `McpNotification` into the channel registered via
//! [`RecordingAgentWithFixture::add_mcp_source`]. Dropping the wrapper
//! flushes the recording to disk; we read it back and assert that **both**
//! notification sources appear in the recorded fixture.

use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::schema::{
    InitializeRequest, NewSessionRequest, PromptRequest, ProtocolVersion, SessionId,
};
use agent_client_protocol::{ConnectTo, Result as AcpResult};
use agent_client_protocol_extras::{
    AgentWithFixture, PlaybackAgent, RecordedSession, RecordingAgent,
};
use model_context_protocol_extras::McpNotification;
use rmcp::model::{LoggingLevel, LoggingMessageNotificationParam};

/// Build a `PlaybackAgent` from inline JSON. The session has three calls:
/// `initialize`, `new_session` (yielding session id `s1`), and `prompt`
/// (whose recorded payload includes a single agent_message_chunk
/// notification).
fn playback_with_one_prompt_notification(path: &PathBuf) -> PlaybackAgent {
    let session = serde_json::json!({
        "calls": [
            {
                "method": "initialize",
                "request": {"protocolVersion": 1},
                "response": {
                    "protocolVersion": 1,
                    "agentCapabilities": {},
                    "authMethods": []
                },
                "notifications": []
            },
            {
                "method": "new_session",
                "request": {"cwd": "/tmp", "mcpServers": []},
                "response": {"sessionId": "s1"},
                "notifications": []
            },
            {
                "method": "prompt",
                "request": {
                    "sessionId": "s1",
                    "prompt": [{"type":"text","text":"hi"}]
                },
                "response": {"stopReason": "end_turn"},
                "notifications": [
                    {
                        "sessionId": "s1",
                        "update": {
                            "sessionUpdate": "agent_message_chunk",
                            "content": {"type":"text","text":"wire-side notif"}
                        }
                    }
                ]
            }
        ]
    });
    std::fs::write(path, serde_json::to_string(&session).unwrap()).unwrap();
    PlaybackAgent::new(path.clone(), "test")
}

/// Helper: extract the `session/update` notification payload at
/// `calls[*].notifications[*]` whose `update.content.text` matches `marker`.
fn find_notification_with_marker<'a>(
    session: &'a RecordedSession,
    marker: &str,
) -> Option<&'a serde_json::Value> {
    for call in &session.calls {
        for notif in &call.notifications {
            if let Some(text) = notif
                .pointer("/update/content/text")
                .and_then(|v| v.as_str())
            {
                if text == marker {
                    return Some(notif);
                }
            }
        }
    }
    None
}

/// Helper: search the recording for any notification whose `level` field
/// matches a captured MCP logging notification.
fn find_mcp_logging_notification<'a>(
    session: &'a RecordedSession,
    expected_level: &str,
) -> Option<&'a serde_json::Value> {
    for call in &session.calls {
        for notif in &call.notifications {
            // McpNotification serializes as either {"Log": {...}} or
            // {"Progress": {...}} — see model-context-protocol-extras.
            if let Some(level) = notif.pointer("/Log/level").and_then(|v| v.as_str()) {
                if level.eq_ignore_ascii_case(expected_level) {
                    return Some(notif);
                }
            }
        }
    }
    None
}

/// `ConnectTo<Client>` adapter that just hands its incoming channel back to
/// the caller. Used to wire up an in-process duplex without going through
/// the full Builder/connect_with handshake. The test wires its own
/// `Channel::duplex` between a [`PlaybackAgent`] (the inner) and the
/// [`RecordingAgent`] middleware, so it doesn't need a separate transport
/// adapter — but since [`RecordingAgent::with_notifications`] takes the inner
/// directly, we don't need ChannelExposer here either.
///
/// Kept around as documentation: in fixture-style tests the wiring is
/// `RecordingAgent::with_notifications(playback, ...)` and the wrapper does
/// the duplex spin-up.
#[allow(dead_code)]
struct ChannelExposer;

#[allow(dead_code)]
impl ConnectTo<agent_client_protocol::Agent> for ChannelExposer {
    async fn connect_to(
        self,
        _agent: impl ConnectTo<<agent_client_protocol::Agent as agent_client_protocol::Role>::Counterpart>,
    ) -> AcpResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn recording_with_notifications_captures_wire_and_mcp_sources() {
    // Lay down a fresh fixture file location.
    let dir = tempfile::tempdir().unwrap();
    let inner_fixture = dir.path().join("inner.json");
    let recording_path = dir.path().join("recording.json");

    // Inner: a PlaybackAgent that will serve our three pre-canned calls.
    let inner = playback_with_one_prompt_notification(&inner_fixture);

    // Wire up a side-channel for SessionNotification — this exercises the
    // `with_notifications` constructor even though our PlaybackAgent doesn't
    // emit through the side channel itself. The test only needs the channel
    // alive for the duration of the call so the drain task is created and
    // its `SourceHandle` lands on the wrapper.
    let (_session_notif_tx, session_notif_rx) =
        tokio::sync::broadcast::channel::<agent_client_protocol::schema::SessionNotification>(8);

    let wrapper = RecordingAgent::with_notifications(
        inner,
        recording_path.clone(),
        "test",
        session_notif_rx,
    )
    .await
    .expect("wrapper constructs");

    assert_eq!(wrapper.agent_type(), "test");

    // Register an MCP capture source. We feed it directly with a synthetic
    // `McpNotification` — in production this receiver comes from
    // `start_test_mcp_server_with_capture()`, but for a deterministic test
    // we cut out the proxy and inject the broadcast directly so the
    // assertion doesn't depend on an external server's timing.
    let (mcp_tx, mcp_rx) = tokio::sync::broadcast::channel::<McpNotification>(8);
    wrapper.add_mcp_source(mcp_rx);

    // Drive the connection through the three recorded calls so the wire
    // side has something to record.
    let conn = wrapper.connection();

    let _init = conn
        .send_request(InitializeRequest::new(ProtocolVersion::V1))
        .block_task()
        .await
        .expect("initialize");

    let new_session = conn
        .send_request(NewSessionRequest::new(PathBuf::from("/tmp")))
        .block_task()
        .await
        .expect("new_session");
    let session_id: SessionId = new_session.session_id;

    // Push the MCP notification through the capture source before the
    // prompt fires so the routing has somewhere to land.
    mcp_tx
        .send(McpNotification::Log(LoggingMessageNotificationParam {
            level: LoggingLevel::Info,
            logger: Some("test-mcp-server".to_string()),
            data: serde_json::json!({"message": "captured-from-mcp"}),
        }))
        .expect("mcp send");

    // Drive the prompt — the inner PlaybackAgent will fire its recorded
    // `session/update` notification on the way back, which the recording
    // wrap observes as wire-side traffic.
    let _prompt_resp = conn
        .send_request(PromptRequest::new(
            session_id,
            vec![agent_client_protocol::schema::ContentBlock::Text(
                agent_client_protocol::schema::TextContent::new("hi"),
            )],
        ))
        .block_task()
        .await
        .expect("prompt");

    // Give the drain tasks a beat to observe the queued mcp notification
    // before we tear the wrapper down. Drop closes the duplex; the inner
    // `connect_to` future winds down; the recording flushes.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Drop the wrapper to flush the recording.
    drop(wrapper);

    // Recording is flushed during connect_to teardown — wait briefly for
    // the file-system rename to land.
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Read the recorded fixture and assert both notification sources made
    // it in.
    let contents = std::fs::read_to_string(&recording_path)
        .expect("recording file should exist after drop");
    let session: RecordedSession =
        serde_json::from_str(&contents).expect("recording deserializes as RecordedSession");

    // The wire-side notification should be present, attached to the prompt
    // call (it carried sessionId == s1 which matches the prompt's session).
    let wire_notif = find_notification_with_marker(&session, "wire-side notif")
        .expect("wire-side notification should be in the recording");
    assert_eq!(
        wire_notif["sessionId"], "s1",
        "wire notification preserves sessionId"
    );

    // The MCP-side notification should also be present, attached to the
    // last prompt as a fallback (no sessionId on McpNotification).
    let mcp_notif = find_mcp_logging_notification(&session, "info")
        .expect("MCP logging notification should be in the recording");
    assert_eq!(
        mcp_notif["Log"]["data"]["message"], "captured-from-mcp",
        "MCP notification carries our marker"
    );

    // Keep `mcp_tx` alive long enough to ensure the drain task can be
    // signalled to exit naturally — the wrapper drop already aborted the
    // drain task, but holding the sender pins down the broadcast for the
    // duration of the test.
    let _ = Arc::new(mcp_tx);
}
