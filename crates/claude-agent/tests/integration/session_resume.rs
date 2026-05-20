//! Tests for `session/resume` and `session/load`, backed by the shared
//! `SessionStore` and the claude CLI's own `--resume`.
//!
//! claude-agent restores a session from a durable
//! [`SessionRecord`](agent_client_protocol_extras::SessionRecord):
//!
//! - `session/resume` restores agent state and returns — it MUST NOT replay
//!   history.
//! - `session/load` restores agent state, then replays the recorded
//!   conversation as `session/update` notifications, then returns.
//!
//! Both share state restoration; `load` is `resume` plus replay.
//!
//! These tests exercise the deterministic part of both handlers — the durable
//! record lookup and the salvaged expiration / integrity / capability gating —
//! through claude-agent's public surface. They stop short of driving a real
//! `claude --resume` against a live transcript, matching the rest of this
//! crate's integration suite, which never spawns the claude CLI. The record
//! validation runs *before* any CLI spawn, so every assertion here is
//! deterministic.
//!
//! Covered:
//!
//! 1. `initialize` advertises `sessionCapabilities.resume`.
//! 2. `session/resume` and `session/load` reject a session with no persisted
//!    record.
//! 3. They reject an expired record.
//! 4. They reject a corrupt record (future timestamp).
//! 5. An opaque non-ULID session id fails as a resume/lookup error, never as a
//!    session-id format rejection.
//! 6. `replay_record_updates` — the salvaged `session/load` replay loop —
//!    streams every recorded update back to the client as a tagged
//!    `historical_replay` notification, and is a no-op for an empty record.
//! 7. `rehydrate_in_memory_session` reconstructs the live in-memory session
//!    (cwd, MCP servers, and update history) from the durable record.
//!
//! The replay loop's consecutive-failure abort branch is intentionally *not*
//! covered: `NotificationSender::send_update` is infallible (see the comment
//! on that branch in `session_resume.rs`), so it cannot be driven without a
//! failing sender, and the concrete `notification_sender` field admits no
//! injected fake. Only the reachable success path is exercised here.

use agent_client_protocol::schema::{
    ContentBlock, ContentChunk, InitializeRequest, LoadSessionRequest, McpServer, McpServerHttp,
    ResumeSessionRequest, SessionId, SessionUpdate, TextContent,
};
use agent_client_protocol_extras::{SessionRecord, SessionStore};
use claude_agent::session::SessionId as AgentSessionId;
use claude_agent::{config::AgentConfig, ClaudeAgent};
use serial_test::serial;
use std::path::PathBuf;

/// Run `body` with `XDG_STATE_HOME` pointed at a fresh temp directory, so the
/// `SessionStore` reads and writes an isolated `acp/` state tree. The previous
/// value is restored afterwards.
///
/// Callers must be `#[serial]`: this mutates the process-global
/// `XDG_STATE_HOME` environment variable.
async fn with_temp_state<F, Fut, R>(body: F) -> R
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let temp = tempfile::tempdir().unwrap();
    let previous = std::env::var_os("XDG_STATE_HOME");
    // SAFETY: callers are `#[serial]`, so no other thread reads or writes the
    // env var concurrently; the previous value is restored below.
    std::env::set_var("XDG_STATE_HOME", temp.path());
    let result = body().await;
    match previous {
        Some(value) => std::env::set_var("XDG_STATE_HOME", value),
        None => std::env::remove_var("XDG_STATE_HOME"),
    }
    drop(temp);
    result
}

/// A ULID-shaped session id usable as a `session.json` directory name.
const SESSION_ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

/// Build a persisted-ready `SessionRecord` for `SESSION_ID` with the given
/// last-activity timestamp.
fn record_at(updated_at: &str) -> SessionRecord {
    SessionRecord::new(SESSION_ID, PathBuf::from("/work/resume"), updated_at)
}

/// `initialize` advertises `sessionCapabilities.resume` alongside the existing
/// `list` and `load_session` capabilities.
#[tokio::test]
#[serial]
async fn initialize_advertises_session_resume_capability() {
    with_temp_state(|| async {
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let response = agent
            .initialize(InitializeRequest::new(1.into()))
            .await
            .unwrap();

        assert!(
            response
                .agent_capabilities
                .session_capabilities
                .resume
                .is_some(),
            "agent must advertise sessionCapabilities.resume"
        );
        assert!(
            response
                .agent_capabilities
                .session_capabilities
                .list
                .is_some(),
            "sessionCapabilities.list must still be advertised"
        );
        assert!(
            response.agent_capabilities.load_session,
            "load_session capability must still be advertised"
        );
    })
    .await;
}

/// `session/resume` fails when no record is persisted for the session id.
/// The failure is a lookup error, not a session-id format rejection.
#[tokio::test]
#[serial]
async fn resume_session_rejects_missing_record() {
    with_temp_state(|| async {
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let request =
            ResumeSessionRequest::new(SessionId::new(SESSION_ID), PathBuf::from("/work/resume"));
        let error = agent
            .resume_session(request)
            .await
            .expect_err("resume of a session with no persisted record must fail");

        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::InvalidParams,
            "missing record is a lookup failure"
        );
    })
    .await;
}

/// `session/load` fails when no record is persisted for the session id.
#[tokio::test]
#[serial]
async fn load_session_rejects_missing_record() {
    with_temp_state(|| async {
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let request =
            LoadSessionRequest::new(SessionId::new(SESSION_ID), PathBuf::from("/work/resume"));
        let error = agent
            .load_session(request)
            .await
            .expect_err("load of a session with no persisted record must fail");

        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::InvalidParams,
            "missing record is a lookup failure"
        );
    })
    .await;
}

/// `session/resume` rejects a record whose last activity is older than the
/// maximum resumable age. The record exists, so this is an expiry failure.
#[tokio::test]
#[serial]
async fn resume_session_rejects_expired_record() {
    with_temp_state(|| async {
        // Persist a record last touched well over a day ago.
        SessionStore::new()
            .persist(&record_at("2000-01-01T00:00:00Z"))
            .unwrap();

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let request =
            ResumeSessionRequest::new(SessionId::new(SESSION_ID), PathBuf::from("/work/resume"));
        let error = agent
            .resume_session(request)
            .await
            .expect_err("resume of an expired record must fail");

        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(
            error.message.contains("expired"),
            "expiry error should explain the record is too old: {}",
            error.message
        );
    })
    .await;
}

/// `session/load` rejects a record whose last activity is older than the
/// maximum resumable age.
#[tokio::test]
#[serial]
async fn load_session_rejects_expired_record() {
    with_temp_state(|| async {
        SessionStore::new()
            .persist(&record_at("2000-01-01T00:00:00Z"))
            .unwrap();

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let request =
            LoadSessionRequest::new(SessionId::new(SESSION_ID), PathBuf::from("/work/resume"));
        let error = agent
            .load_session(request)
            .await
            .expect_err("load of an expired record must fail");

        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(
            error.message.contains("expired"),
            "expiry error should explain the record is too old: {}",
            error.message
        );
    })
    .await;
}

/// `session/resume` rejects a corrupt record — here, one whose `updated_at`
/// timestamp is in the future, the salvaged integrity check.
#[tokio::test]
#[serial]
async fn resume_session_rejects_corrupt_record() {
    with_temp_state(|| async {
        // A timestamp far in the future cannot be a real last-activity time.
        SessionStore::new()
            .persist(&record_at("2999-01-01T00:00:00Z"))
            .unwrap();

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let request =
            ResumeSessionRequest::new(SessionId::new(SESSION_ID), PathBuf::from("/work/resume"));
        let error = agent
            .resume_session(request)
            .await
            .expect_err("resume of a corrupt record must fail");

        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(
            error.message.contains("corrupt"),
            "integrity error should flag the record as corrupt: {}",
            error.message
        );
    })
    .await;
}

/// `session/load` rejects a corrupt record whose `updated_at` is in the future.
#[tokio::test]
#[serial]
async fn load_session_rejects_corrupt_record() {
    with_temp_state(|| async {
        SessionStore::new()
            .persist(&record_at("2999-01-01T00:00:00Z"))
            .unwrap();

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let request =
            LoadSessionRequest::new(SessionId::new(SESSION_ID), PathBuf::from("/work/resume"));
        let error = agent
            .load_session(request)
            .await
            .expect_err("load of a corrupt record must fail");

        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert!(
            error.message.contains("corrupt"),
            "integrity error should flag the record as corrupt: {}",
            error.message
        );
    })
    .await;
}

/// An opaque, non-ULID session id is not rejected on format: it is accepted as
/// a valid session id and fails only because no record can be resumed for it.
///
/// This is the opaque-session-id contract — the id is an opaque string, and a
/// non-ULID id that cannot be `--resume`d surfaces as a lookup/resume failure,
/// never a format rejection.
#[tokio::test]
#[serial]
async fn resume_session_treats_non_ulid_id_as_lookup_failure() {
    with_temp_state(|| async {
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        // "my-session" is a perfectly valid opaque session id and a safe path
        // component, but it is not a ULID — there is simply no record for it.
        let request =
            ResumeSessionRequest::new(SessionId::new("my-session"), PathBuf::from("/work/resume"));
        let error = agent
            .resume_session(request)
            .await
            .expect_err("resume of an unknown opaque id must fail");

        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::InvalidParams,
            "a non-ULID id must fail as a lookup/resume error, not a format rejection"
        );
        assert!(
            error.message.contains("no persisted session record"),
            "the failure must be a record lookup miss, not an id-format complaint: {}",
            error.message
        );
    })
    .await;
}

/// Build a `UserMessageChunk` `SessionUpdate` carrying the given text, for
/// seeding a record's replayable `updates` stream.
fn text_update(text: &str) -> SessionUpdate {
    SessionUpdate::UserMessageChunk(ContentChunk::new(ContentBlock::Text(TextContent::new(
        text.to_string(),
    ))))
}

/// Drain every `SessionNotification` currently buffered on a broadcast
/// receiver into a `Vec`, stopping at the first `Empty` (no more pending).
fn drain_notifications(
    rx: &mut tokio::sync::broadcast::Receiver<agent_client_protocol::schema::SessionNotification>,
) -> Vec<agent_client_protocol::schema::SessionNotification> {
    let mut collected = Vec::new();
    while let Ok(notification) = rx.try_recv() {
        collected.push(notification);
    }
    collected
}

/// `replay_record_updates` — the salvaged `session/load` replay loop — streams
/// every recorded `SessionUpdate` back to the client as a `session/update`
/// notification, in order, each tagged as a `historical_replay` with its
/// index and the total count.
///
/// This exercises the replay loop directly, without driving a live
/// `claude --resume`: the loop needs only a `SessionRecord` and the agent's
/// notification channel.
#[tokio::test]
#[serial]
async fn replay_record_updates_streams_every_update_as_a_notification() {
    with_temp_state(|| async {
        let (agent, mut rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let mut record = record_at("2026-05-18T12:00:00Z");
        record.updates = vec![
            text_update("first"),
            text_update("second"),
            text_update("third"),
        ];

        agent
            .replay_record_updates(&record)
            .await
            .expect("replaying a well-formed record must succeed");

        let notifications = drain_notifications(&mut rx);
        assert_eq!(
            notifications.len(),
            3,
            "every recorded update must be replayed as one notification"
        );

        for (index, notification) in notifications.iter().enumerate() {
            assert_eq!(
                notification.session_id.0.to_string(),
                SESSION_ID,
                "replayed notification must carry the record's session id"
            );
            let meta = notification
                .meta
                .as_ref()
                .expect("a replayed update must be tagged with replay metadata");
            assert_eq!(
                meta.get("message_type").and_then(|v| v.as_str()),
                Some("historical_replay"),
                "replayed updates must be marked as historical, not live output"
            );
            assert_eq!(
                meta.get("message_index").and_then(|v| v.as_u64()),
                Some(index as u64),
                "each replayed update must carry its position in the stream"
            );
            assert_eq!(
                meta.get("total_messages").and_then(|v| v.as_u64()),
                Some(3),
                "each replayed update must carry the total stream length"
            );
        }
    })
    .await;
}

/// `replay_record_updates` is a no-op for a record with no updates: it sends
/// nothing and returns `Ok` — a `session/resume`-shaped record carries no
/// history to stream.
#[tokio::test]
#[serial]
async fn replay_record_updates_is_a_noop_for_an_empty_record() {
    with_temp_state(|| async {
        let (agent, mut rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let record = record_at("2026-05-18T12:00:00Z");
        assert!(record.updates.is_empty(), "fixture record has no updates");

        agent
            .replay_record_updates(&record)
            .await
            .expect("replaying an empty record must succeed");

        assert!(
            drain_notifications(&mut rx).is_empty(),
            "an empty record must replay nothing to the client"
        );
    })
    .await;
}

/// `rehydrate_in_memory_session` reconstructs the live in-memory session from
/// the durable record after a process restart: the working directory, the MCP
/// server configuration, and the accumulated update history are all restored
/// into the `SessionManager` so the next `session/prompt` finds the session.
#[tokio::test]
#[serial]
async fn rehydrate_in_memory_session_restores_cwd_mcp_servers_and_updates() {
    with_temp_state(|| async {
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let mut record = SessionRecord::new(
            SESSION_ID,
            PathBuf::from("/work/rehydrate"),
            "2026-05-18T12:00:00Z",
        );
        record.mcp_servers = vec![McpServer::Http(McpServerHttp::new(
            "rehydrate-mcp",
            "https://mcp.example/sse",
        ))];
        record.updates = vec![text_update("hello"), text_update("world")];

        let session_id = AgentSessionId::parse(SESSION_ID).expect("SESSION_ID is a valid ULID");
        agent
            .rehydrate_in_memory_session(session_id, &record)
            .expect("rehydrating a well-formed record must succeed");

        let session = agent
            .session_manager()
            .get_session(&session_id)
            .expect("session manager read must not fail")
            .expect("the rehydrated session must be present in the in-memory cache");

        assert_eq!(
            session.cwd,
            PathBuf::from("/work/rehydrate"),
            "the record's working directory must be restored"
        );
        assert_eq!(
            session.mcp_servers.len(),
            1,
            "the record's MCP server configuration must be restored"
        );
        assert!(
            session.mcp_servers[0].contains("rehydrate-mcp"),
            "the restored MCP server must carry the record's server name: {}",
            session.mcp_servers[0]
        );
        assert_eq!(
            session.context.len(),
            2,
            "every recorded update must be restored as a session message"
        );
    })
    .await;
}
