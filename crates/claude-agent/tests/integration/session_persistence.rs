//! Tests for durable session persistence via the shared `SessionStore`.
//!
//! claude-agent persists a [`SessionRecord`] to the shared
//! [`SessionStore`](agent_client_protocol_extras::SessionStore) at the end of
//! each turn. This gives sessions durable, cross-restart persistence — they
//! survive the process exiting — and makes them answerable by the ACP
//! `session/list` method.
//!
//! These tests exercise that wiring through claude-agent's public surface:
//!
//! 1. `session/list` returns persisted sessions, newest first.
//! 2. The `cwd` filter narrows results to one working directory.
//! 3. Cursor pagination walks every persisted session exactly once.
//! 4. Records round-trip through a simulated process restart — a record
//!    persisted by one `SessionStore` is read back by a fresh one, exactly as
//!    a new process would.
//! 5. `initialize` advertises the `sessionCapabilities.list` capability.

use agent_client_protocol::schema::{
    ContentBlock, ContentChunk, InitializeRequest, ListSessionsRequest, SessionUpdate, TextContent,
};
use agent_client_protocol_extras::{SessionRecord, SessionStore};
use claude_agent::session::{Message, MessageRole};
use claude_agent::{config::AgentConfig, ClaudeAgent};
use serial_test::serial;
use std::path::{Path, PathBuf};

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

/// Build a `SessionRecord` with one user-message update for the given id and
/// cwd, so a persisted session has realistic listing metadata.
fn record_with_message(id: &str, cwd: &str, message: &str) -> SessionRecord {
    let mut record = SessionRecord::new(id, PathBuf::from(cwd), "2026-05-18T12:00:00Z");
    record.title = Some(message.to_string());
    record
        .updates
        .push(SessionUpdate::UserMessageChunk(ContentChunk::new(
            ContentBlock::Text(TextContent::new(message.to_string())),
        )));
    record
}

/// Collect the session-id strings from a `ListSessionsResponse`.
fn ids(response: &agent_client_protocol::schema::ListSessionsResponse) -> Vec<String> {
    response
        .sessions
        .iter()
        .map(|s| s.session_id.0.to_string())
        .collect()
}

/// `session/list` with no filter returns every persisted session, newest
/// (highest ULID) first.
#[tokio::test]
#[serial]
async fn session_list_returns_persisted_sessions_newest_first() {
    with_temp_state(|| async {
        let store = SessionStore::new();
        for id in ["01AAA0000000000000000000A0", "01BBB0000000000000000000B0"] {
            store
                .persist(&record_with_message(id, "/work/x", "hello"))
                .unwrap();
        }

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let response = agent
            .list_sessions(ListSessionsRequest::new())
            .await
            .unwrap();

        assert_eq!(
            ids(&response),
            vec![
                "01BBB0000000000000000000B0".to_string(),
                "01AAA0000000000000000000A0".to_string(),
            ]
        );
        assert!(response.next_cursor.is_none());
    })
    .await;
}

/// `session/list` honors the `cwd` filter, returning only sessions whose
/// working directory matches exactly.
#[tokio::test]
#[serial]
async fn session_list_applies_cwd_filter() {
    with_temp_state(|| async {
        let store = SessionStore::new();
        store
            .persist(&record_with_message(
                "01CCC0000000000000000000C0",
                "/work/keep",
                "keep me",
            ))
            .unwrap();
        store
            .persist(&record_with_message(
                "01DDD0000000000000000000D0",
                "/work/skip",
                "skip me",
            ))
            .unwrap();
        store
            .persist(&record_with_message(
                "01EEE0000000000000000000E0",
                "/work/keep",
                "keep me too",
            ))
            .unwrap();

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let response = agent
            .list_sessions(ListSessionsRequest::new().cwd(PathBuf::from("/work/keep")))
            .await
            .unwrap();

        assert_eq!(
            ids(&response),
            vec![
                "01EEE0000000000000000000E0".to_string(),
                "01CCC0000000000000000000C0".to_string(),
            ]
        );
    })
    .await;
}

/// `session/list` paginates: following the `next_cursor` walks every persisted
/// session exactly once, and the final page carries no cursor.
#[tokio::test]
#[serial]
async fn session_list_paginates_with_cursor() {
    with_temp_state(|| async {
        let store = SessionStore::new();
        // Persist more sessions than a single page of SESSION_LIST_PAGE_SIZE (50)
        // so that pagination is actually exercised.
        let mut expected = Vec::new();
        for i in 0..130 {
            let id = format!("01AAA00000000000000000{:04}", i);
            store
                .persist(&record_with_message(&id, "/work/p", "paged"))
                .unwrap();
            expected.push(id);
        }
        // Newest (highest ULID) first.
        expected.sort_unstable_by(|a, b| b.cmp(a));

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let mut collected = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let request = match &cursor {
                Some(c) => ListSessionsRequest::new().cursor(c.clone()),
                None => ListSessionsRequest::new(),
            };
            let response = agent.list_sessions(request).await.unwrap();
            collected.extend(ids(&response));
            match response.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }

        assert_eq!(
            collected, expected,
            "pagination should visit every persisted session exactly once"
        );
    })
    .await;
}

/// A `SessionRecord` persisted by one `SessionStore` is read back intact by a
/// fresh `SessionStore` — exactly as a brand-new process would after a restart.
/// This is the cross-restart durability that did not exist before.
#[tokio::test]
#[serial]
async fn session_record_round_trips_across_simulated_restart() {
    with_temp_state(|| async {
        let id = "01FFF0000000000000000000F0";

        // "First process": persist a record.
        {
            let store = SessionStore::new();
            store
                .persist(&record_with_message(id, "/work/restart", "before restart"))
                .unwrap();
        }

        // "Second process": a fresh store, as if the process had restarted.
        let restarted_store = SessionStore::new();
        let loaded = restarted_store
            .load(id)
            .unwrap()
            .expect("record should survive the simulated restart");
        assert_eq!(loaded.session_id, id);
        assert_eq!(loaded.cwd, Path::new("/work/restart"));
        assert_eq!(loaded.updates.len(), 1);

        // And the agent's `session/list` sees it after the restart.
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let response = agent
            .list_sessions(ListSessionsRequest::new())
            .await
            .unwrap();
        assert_eq!(ids(&response), vec![id.to_string()]);
    })
    .await;
}

/// `session/list` against an empty store is an empty page, not an error.
#[tokio::test]
#[serial]
async fn session_list_with_no_sessions_is_empty() {
    with_temp_state(|| async {
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let response = agent
            .list_sessions(ListSessionsRequest::new())
            .await
            .unwrap();
        assert!(response.sessions.is_empty());
        assert!(response.next_cursor.is_none());
    })
    .await;
}

/// `initialize` advertises the `sessionCapabilities.list` capability alongside
/// the existing `load_session` capability.
#[tokio::test]
#[serial]
async fn initialize_advertises_session_list_capability() {
    with_temp_state(|| async {
        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

        let request = InitializeRequest::new(1.into());
        let response = agent.initialize(request).await.unwrap();

        assert!(
            response
                .agent_capabilities
                .session_capabilities
                .list
                .is_some(),
            "agent must advertise sessionCapabilities.list"
        );
        assert!(
            response.agent_capabilities.load_session,
            "load_session capability must still be advertised"
        );
    })
    .await;
}

/// `maybe_generate_session_title` generates a title from the first user
/// message after the first meaningful exchange, stores it on the live
/// session, and persists it so `session/list` reflects it.
///
/// This exercises the title-generation path through claude-agent's public
/// surface — it lives here, in a real integration target, because the crate's
/// `[lib] test = false` makes `#[cfg(test)]` lib modules dead code.
#[tokio::test]
#[serial]
async fn maybe_generate_session_title_sets_and_persists_title() {
    with_temp_state(|| async {
        let cwd = tempfile::tempdir().unwrap();
        let cwd_path = cwd.path().to_path_buf();

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let session_id = agent
            .session_manager()
            .create_session(cwd_path.clone(), None)
            .unwrap();
        agent
            .session_manager()
            .update_session(&session_id, |session| {
                session.add_message(Message::new(
                    MessageRole::User,
                    "Add dark mode to the settings page".to_string(),
                ));
                session.add_message(Message::new(
                    MessageRole::Assistant,
                    "Sure, here is how.".to_string(),
                ));
            })
            .unwrap();

        agent.maybe_generate_session_title(&session_id);

        // Generation is dispatched to a detached task; give it a moment.
        let title = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let session = agent
                    .session_manager()
                    .get_session(&session_id)
                    .unwrap()
                    .unwrap();
                if let Some(title) = session.title {
                    break title;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("title should be generated within the timeout");

        assert_eq!(title, "Add dark mode to the settings page");

        let loaded = SessionStore::new()
            .load(&session_id.to_string())
            .unwrap()
            .expect("record should be persisted with the title");
        assert_eq!(
            loaded.title.as_deref(),
            Some("Add dark mode to the settings page")
        );
    })
    .await;
}

/// `maybe_generate_session_title` does not generate a title before the first
/// agent response — a user message alone is not a full exchange.
#[tokio::test]
#[serial]
async fn maybe_generate_session_title_waits_for_first_exchange() {
    with_temp_state(|| async {
        let cwd = tempfile::tempdir().unwrap();
        let cwd_path = cwd.path().to_path_buf();

        let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();
        let session_id = agent
            .session_manager()
            .create_session(cwd_path.clone(), None)
            .unwrap();
        agent
            .session_manager()
            .update_session(&session_id, |session| {
                session.add_message(Message::new(MessageRole::User, "just a prompt".to_string()));
            })
            .unwrap();

        agent.maybe_generate_session_title(&session_id);

        // Allow any (incorrectly) spawned task time to run.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let session = agent
            .session_manager()
            .get_session(&session_id)
            .unwrap()
            .unwrap();
        assert!(
            session.title.is_none(),
            "no title should be generated before the first agent response"
        );
    })
    .await;
}
