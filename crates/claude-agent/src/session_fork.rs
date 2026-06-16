//! Session forking over ACP: a new session seeded from a parent session's
//! conversation and the claude CLI's persisted transcript.
//!
//! This module implements claude-agent's side of the backend-agnostic
//! extension contract in [`agent_client_protocol_extras::session_fork`] — the
//! same three `ext_method`s llama-agent serves, so the validators-pool client
//! drives either backend through one code path:
//!
//! - `session/fork` ([`ClaudeAgent::fork_session`]) — clone the parent's
//!   in-memory [`Session`](crate::session::Session) under a fresh ULID and
//!   spawn the child's claude CLI process with `--resume <parent-uuid>
//!   --fork-session --session-id <child-uuid>`, the CLI's native
//!   conversation-fork primitive.
//! - `session/state_status` ([`ClaudeAgent::session_state_status`]) — report
//!   whether a session has restorable state to fork from. For the claude
//!   backend, "state" is the CLI's persisted transcript: it is forkable once
//!   the session has completed a turn (and the agent is not running in
//!   ephemeral mode, which disables CLI transcript persistence). The CLI does
//!   not expose token counts, so `prompt_tokens`/`bytes` are `None` — the
//!   contract explicitly allows that.
//! - `session/pin` ([`ClaudeAgent::pin_session`]) — a no-op: the CLI's
//!   transcript store has no cache eviction, so there is nothing to pin. Per
//!   the contract, the response reports the *effective* pin state (`false`)
//!   rather than pretending.
//!
//! # Prefix caching
//!
//! A fork replays the identical conversation prefix, so Anthropic's
//! server-side prompt caching covers the shared prefix automatically.
//! claude-agent delegates all API request construction to the claude CLI
//! (which manages prompt caching itself); no custom caching layer exists or
//! is needed here.

use agent_client_protocol_extras::{
    SessionErrorKind, SessionForkRequest, SessionForkResponse, SessionPinRequest,
    SessionPinResponse, SessionStateStatusRequest, SessionStateStatusResponse,
};

use crate::acp_error::session_error;
use crate::agent::ClaudeAgent;
use crate::claude::ForkAttachError;
use crate::session::{Session, SessionId};

/// Map a fork state-attach failure onto the shared wire contract, preserving
/// the distinction [`ForkAttachError`] carries.
///
/// The CLI cleanly rejecting the fork is the genuine "parent state
/// unavailable" condition and maps onto `invalid_params` with `data.error ==`
/// [`SessionErrorKind::ForkParentStateUnavailable`] — the permanent signal
/// telling the client to fall back to `session/new` + full prompt. An environment
/// failure launching the process (claude binary missing, spawn I/O) says
/// nothing about the parent's state and maps onto a retryable `-32603`
/// internal error instead — a client must never re-prime futilely against a
/// broken CLI install.
fn fork_attach_error(
    parent_session_id: &str,
    failure: ForkAttachError,
) -> agent_client_protocol::Error {
    match failure {
        ForkAttachError::Rejected { detail } => session_error(
            parent_session_id,
            SessionErrorKind::ForkParentStateUnavailable,
            detail,
        ),
        ForkAttachError::Spawn(e) => {
            tracing::error!(
                "Failed to spawn forked claude process for parent {parent_session_id}: {e}"
            );
            crate::acp_error::internal_error(format!(
                "forked claude process could not be spawned for parent {parent_session_id}: {e}"
            ))
        }
    }
}

/// Clone the parent's conversation under the fresh child id: fresh
/// timestamps, no title. The context, cwd, MCP server config, capabilities,
/// and current mode all carry over — the child continues the parent's
/// conversation independently.
fn clone_child_session(parent: &Session, child_id: SessionId) -> Session {
    let now = std::time::SystemTime::now();
    let mut child = parent.clone();
    child.id = child_id;
    child.created_at = now;
    child.last_accessed = now;
    child.title = None;
    child
}

impl ClaudeAgent {
    /// Handle the `session/fork` extension: create a new session seeded from
    /// `parent_session_id`'s conversation and the claude CLI's persisted
    /// transcript.
    ///
    /// The child's CLI process is spawned FIRST with `--resume <parent-uuid>
    /// --fork-session --session-id <child-uuid>` — the fallible
    /// state-attaching step ("never fork blind") — so a parent whose
    /// transcript cannot seed a fork never half-creates a child session. On
    /// success the child:
    ///
    /// - holds a clone of the parent's conversation, cwd, and MCP config
    ///   under a fresh ULID (fresh timestamps, no title);
    /// - owns its own CLI process whose conversation is the CLI's native fork
    ///   of the parent's transcript, persisted under the child's own
    ///   deterministic UUID — fully independent of the parent;
    /// - has its transcript recorder wired and a durable
    ///   [`SessionRecord`](agent_client_protocol_extras::SessionRecord)
    ///   persisted, the same lifecycle a turn-completing session gets.
    ///
    /// `prefix_tokens` is `None`: the CLI exposes no prompt-token counts (the
    /// contract allows that). Under the headless test seam
    /// (`config.spawn_claude_on_new_session == false`, the same seam
    /// `session/new` honors) no CLI process is spawned.
    ///
    /// # Errors
    ///
    /// `invalid_params` with `data.error` distinguishing the failure:
    /// [`SessionErrorKind::ForkParentNotFound`] when the parent session does not
    /// exist, [`SessionErrorKind::ForkParentStateUnavailable`] when it exists
    /// but has no
    /// restorable transcript — it has not completed a turn, the agent runs in
    /// ephemeral mode (no CLI persistence), or the CLI cleanly rejected the
    /// fork. A session-store failure or an environment failure launching the
    /// forked CLI process (binary missing, spawn I/O) is a retryable
    /// `-32603` internal error instead — never `fork_parent_not_found` or
    /// `fork_parent_state_unavailable` (see [`fork_attach_error`]).
    pub async fn fork_session(
        &self,
        request: SessionForkRequest,
    ) -> Result<SessionForkResponse, agent_client_protocol::Error> {
        let parent = self.resolve_fork_parent(&request)?;

        // Attach the parent's persisted transcript FIRST: spawning the forked
        // CLI process is the fallible gate, and failing here leaves no
        // half-created child behind.
        let child_id = SessionId::new();
        if self.config.spawn_claude_on_new_session {
            self.spawn_forked_process(&parent, child_id)
                .await
                .map_err(|failure| fork_attach_error(&request.parent_session_id, failure))?;
        }

        let child_session = clone_child_session(&parent, child_id);
        if let Err(e) = self.session_manager.restore_session(child_session) {
            // Roll back the forked CLI process so the failed fork leaves
            // nothing behind.
            if self.config.spawn_claude_on_new_session {
                if let Err(term) = self.claude_client.terminate_session(&child_id).await {
                    tracing::warn!(
                        "Failed to roll back forked process for session {}: {}",
                        child_id,
                        term
                    );
                }
            }
            return Err(crate::acp_error::internal_error(format!(
                "forked session could not be stored: {e}"
            )));
        }

        // The same per-session lifecycle a new session gets: transcript
        // recorder plus a durable record so the fork survives a restart and
        // answers `session/list`.
        self.wire_raw_message_manager(&child_id);
        self.persist_session_record(&child_id);

        tracing::info!(
            "Forked session {} from parent {} ({} messages cloned)",
            child_id,
            request.parent_session_id,
            parent.context.len()
        );

        Ok(SessionForkResponse {
            session_id: child_id.to_string(),
            state_attached: true,
            prefix_tokens: None,
        })
    }

    /// Resolve the fork parent and confirm it has restorable state.
    ///
    /// Distinguishes "the parent genuinely does not exist" (the permanent
    /// [`SessionErrorKind::ForkParentNotFound`] fallback signal — including ids
    /// this agent never minted, per the crate's opaque-id convention) from a
    /// session-store FAILURE (lock poisoning), which is a retryable internal
    /// error. A parent that exists but has no restorable transcript — no
    /// completed turn yet, or ephemeral mode disabled CLI persistence —
    /// reports [`SessionErrorKind::ForkParentStateUnavailable`] so the client
    /// can fall back
    /// to a plain `session/new` + full prompt instead of forking blind.
    fn resolve_fork_parent(
        &self,
        request: &SessionForkRequest,
    ) -> Result<Session, agent_client_protocol::Error> {
        let parent_id = &request.parent_session_id;
        let parent = self.resolve_session_with(parent_id, || {
            session_error(
                parent_id,
                SessionErrorKind::ForkParentNotFound,
                format!("fork parent session {parent_id} not found"),
            )
        })?;

        if self.config.claude.ephemeral {
            return Err(session_error(
                parent_id,
                SessionErrorKind::ForkParentStateUnavailable,
                format!(
                    "fork parent session {parent_id} has no restorable state: the agent \
                     runs in ephemeral mode, which disables CLI transcript persistence"
                ),
            ));
        }
        if !Self::has_first_exchange(&parent) {
            return Err(session_error(
                parent_id,
                SessionErrorKind::ForkParentStateUnavailable,
                format!(
                    "fork parent session {parent_id} has not completed a turn; the CLI \
                     has no transcript to fork from"
                ),
            ));
        }

        Ok(parent)
    }

    /// Spawn the child session's claude CLI process forked from the parent's
    /// transcript, mirroring the spawn configuration the resume path uses.
    async fn spawn_forked_process(
        &self,
        parent: &Session,
        child_id: SessionId,
    ) -> std::result::Result<(), ForkAttachError> {
        let spawn_config = crate::claude_process::SpawnConfig::builder()
            .session_id(child_id)
            .acp_session_id(agent_client_protocol::schema::SessionId::new(
                child_id.to_string(),
            ))
            .cwd(parent.cwd.clone())
            .mcp_servers(self.config.mcp_servers.clone())
            .ephemeral(self.config.claude.ephemeral)
            .tools_override(self.config.claude.tools_override.clone())
            .build();

        self.claude_client
            .fork_process(parent.id, spawn_config)
            .await
    }

    /// Handle the `session/state_status` extension: report whether a session
    /// has restorable state to fork from, so a client can confirm before
    /// forking ("never fork blind").
    ///
    /// For the claude backend, state lives in the CLI's persisted transcript:
    /// `saved` is `true` once the session has completed a turn (user prompt +
    /// agent reply), unless the agent runs in ephemeral mode, which disables
    /// CLI persistence. The CLI exposes no token or byte counts, so
    /// `prompt_tokens`/`bytes` are `None` — the contract allows that — and
    /// `pinned` is always `false` (no pinning; see
    /// [`pin_session`](Self::pin_session)).
    ///
    /// # Errors
    ///
    /// `invalid_params` with `data.error ==`
    /// [`SessionErrorKind::SessionStateNotFound`] when the session itself does
    /// not exist.
    pub async fn session_state_status(
        &self,
        request: SessionStateStatusRequest,
    ) -> Result<SessionStateStatusResponse, agent_client_protocol::Error> {
        let session = self.resolve_extension_session(&request.session_id)?;
        let saved = !self.config.claude.ephemeral && Self::has_first_exchange(&session);
        Ok(SessionStateStatusResponse {
            saved,
            prompt_tokens: None,
            bytes: None,
            pinned: false,
        })
    }

    /// Handle the `session/pin` extension as the no-op the contract allows.
    ///
    /// The claude CLI's transcript store has no cache eviction, so there is
    /// nothing to pin a session's state against. Per the contract a backend
    /// without pinning succeeds and reports the *effective* pin state —
    /// `false` — rather than pretending a pin took effect.
    ///
    /// # Errors
    ///
    /// `invalid_params` with `data.error ==`
    /// [`SessionErrorKind::SessionStateNotFound`] when the session does not
    /// exist.
    pub async fn pin_session(
        &self,
        request: SessionPinRequest,
    ) -> Result<SessionPinResponse, agent_client_protocol::Error> {
        self.resolve_extension_session(&request.session_id)?;
        Ok(SessionPinResponse { pinned: false })
    }

    /// Resolve a session for the state-status/pin extensions, mapping a miss
    /// onto the contract's [`SessionErrorKind::SessionStateNotFound`] kind (and
    /// a store failure onto a retryable internal error).
    fn resolve_extension_session(
        &self,
        session_id: &str,
    ) -> Result<Session, agent_client_protocol::Error> {
        self.resolve_session_with(session_id, || {
            session_error(
                session_id,
                SessionErrorKind::SessionStateNotFound,
                format!("session {session_id} not found"),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use agent_client_protocol::schema::{ExtRequest, RawValue};
    use agent_client_protocol_extras::{
        SessionStore, FORK_PARENT_NOT_FOUND, FORK_PARENT_STATE_UNAVAILABLE, SESSION_FORK_METHOD,
        SESSION_PIN_METHOD, SESSION_STATE_NOT_FOUND, SESSION_STATE_STATUS_METHOD,
    };
    use serial_test::serial;
    use std::sync::Arc;

    use super::*;
    use crate::config::AgentConfig;
    use crate::session::{Message, MessageRole};
    use crate::test_support::StateDirGuard;

    /// Agent config for the headless test seam (no real claude CLI is
    /// spawned), optionally in ephemeral mode.
    fn headless_config(ephemeral: bool) -> AgentConfig {
        let mut config = AgentConfig {
            spawn_claude_on_new_session: false,
            ..AgentConfig::default()
        };
        config.claude.ephemeral = ephemeral;
        config
    }

    /// Build a headless agent (the notification receiver rides along so the
    /// sender side stays connected).
    async fn build_agent(
        ephemeral: bool,
    ) -> (
        ClaudeAgent,
        tokio::sync::broadcast::Receiver<agent_client_protocol::schema::SessionNotification>,
    ) {
        ClaudeAgent::new(headless_config(ephemeral))
            .await
            .expect("headless agent")
    }

    /// Create a session and give it a completed exchange (user prompt +
    /// agent reply) — the claude-backend stand-in for a "primed" parent.
    fn primed_parent(agent: &ClaudeAgent, cwd: &Path) -> SessionId {
        let session_id = unprimed_session(agent, cwd);
        agent
            .session_manager
            .update_session(&session_id, |session| {
                session.add_message(Message::new(
                    MessageRole::User,
                    "the corpus under review".to_string(),
                ));
                session.add_message(Message::new(
                    MessageRole::Assistant,
                    "understood".to_string(),
                ));
            })
            .expect("prime parent session");
        session_id
    }

    /// Create a session with no completed turn.
    fn unprimed_session(agent: &ClaudeAgent, cwd: &Path) -> SessionId {
        agent
            .session_manager
            .create_session(cwd.to_path_buf(), None)
            .expect("create session")
    }

    /// A fork of a primed parent clones the conversation under a fresh id,
    /// reports attached state with no token count (the CLI does not track
    /// one), and persists a durable record for the child.
    #[tokio::test]
    #[serial]
    async fn test_fork_clones_parent_history_under_fresh_id() {
        let _state = StateDirGuard::new();
        let cwd = tempfile::tempdir().unwrap();
        let (agent, _rx) = build_agent(false).await;
        let parent_id = primed_parent(&agent, cwd.path());

        let response = agent
            .fork_session(SessionForkRequest {
                parent_session_id: parent_id.to_string(),
            })
            .await
            .expect("fork of a primed parent must succeed");

        assert_ne!(response.session_id, parent_id.to_string());
        assert!(response.state_attached, "fork must report attached state");
        assert_eq!(
            response.prefix_tokens, None,
            "the claude backend tracks no prompt-token counts"
        );

        // The child session holds the parent's conversation under its own id.
        let child_id: SessionId = response.session_id.parse().expect("child ULID");
        let child = agent
            .session_manager
            .get_session(&child_id)
            .expect("lookup")
            .expect("forked session must exist");
        assert_eq!(child.context.len(), 2);
        assert_eq!(child.cwd, cwd.path());

        // The parent is untouched.
        let parent = agent
            .session_manager
            .get_session(&parent_id)
            .expect("lookup")
            .expect("parent must survive the fork");
        assert_eq!(parent.context.len(), 2);

        // The child is durably recorded, like a session/new + completed turn.
        let record = SessionStore::new()
            .load(&response.session_id)
            .expect("store read")
            .expect("forked session must have a durable record");
        assert_eq!(record.updates.len(), 2);
    }

    /// Forking an unknown parent — whether a ULID with no session or an id
    /// this agent never minted — fails with the distinguishable
    /// `fork_parent_not_found` error kind.
    #[tokio::test]
    #[serial]
    async fn test_fork_unknown_parent_errors_distinguishably() {
        let _state = StateDirGuard::new();
        let (agent, _rx) = build_agent(false).await;

        for parent in [SessionId::new().to_string(), "not-a-ulid".to_string()] {
            let error = agent
                .fork_session(SessionForkRequest {
                    parent_session_id: parent.clone(),
                })
                .await
                .expect_err("fork of an unknown parent must error");
            let data = error.data.expect("error must carry structured data");
            assert_eq!(data["error"], FORK_PARENT_NOT_FOUND, "parent {parent}");
        }
    }

    /// Forking a parent that has not completed a turn fails with the
    /// distinguishable `fork_parent_state_unavailable` kind (the CLI has no
    /// transcript to fork yet) and leaves no half-created child behind.
    #[tokio::test]
    #[serial]
    async fn test_fork_unprimed_parent_errors_state_unavailable() {
        let _state = StateDirGuard::new();
        let cwd = tempfile::tempdir().unwrap();
        let (agent, _rx) = build_agent(false).await;
        let parent_id = unprimed_session(&agent, cwd.path());

        let error = agent
            .fork_session(SessionForkRequest {
                parent_session_id: parent_id.to_string(),
            })
            .await
            .expect_err("fork of an unprimed parent must error");

        let data = error.data.expect("error must carry structured data");
        assert_eq!(data["error"], FORK_PARENT_STATE_UNAVAILABLE);
        assert_eq!(
            agent.session_manager.session_count().expect("count"),
            1,
            "a failed fork must leave no half-created child session behind"
        );
    }

    /// In ephemeral mode the CLI persists no transcript, so even a primed
    /// parent has nothing restorable — the fork is rejected with
    /// `fork_parent_state_unavailable`, never silently degraded.
    #[tokio::test]
    #[serial]
    async fn test_fork_ephemeral_agent_errors_state_unavailable() {
        let _state = StateDirGuard::new();
        let cwd = tempfile::tempdir().unwrap();
        let (agent, _rx) = build_agent(true).await;
        let parent_id = primed_parent(&agent, cwd.path());

        let error = agent
            .fork_session(SessionForkRequest {
                parent_session_id: parent_id.to_string(),
            })
            .await
            .expect_err("fork under ephemeral mode must error");

        let data = error.data.expect("error must carry structured data");
        assert_eq!(data["error"], FORK_PARENT_STATE_UNAVAILABLE);
    }

    /// `session/state_status` reports saved truthfully: false before the
    /// first completed turn, true after (with the optional counts absent),
    /// false in ephemeral mode, and an error for an unknown session.
    #[tokio::test]
    #[serial]
    async fn test_state_status_reports_truthfully() {
        let _state = StateDirGuard::new();
        let cwd = tempfile::tempdir().unwrap();
        let (agent, _rx) = build_agent(false).await;

        // Unprimed session: saved == false, not an error.
        let unprimed = unprimed_session(&agent, cwd.path());
        let status = agent
            .session_state_status(SessionStateStatusRequest {
                session_id: unprimed.to_string(),
            })
            .await
            .expect("status of an unprimed session is a valid answer");
        assert!(!status.saved);
        assert_eq!(status.prompt_tokens, None);
        assert!(!status.pinned);

        // Primed session: saved == true; the CLI tracks no counts.
        let primed = primed_parent(&agent, cwd.path());
        let status = agent
            .session_state_status(SessionStateStatusRequest {
                session_id: primed.to_string(),
            })
            .await
            .expect("status");
        assert!(status.saved);
        assert_eq!(status.prompt_tokens, None);
        assert_eq!(status.bytes, None);
        assert!(!status.pinned);

        // Unknown session id → error.
        let error = agent
            .session_state_status(SessionStateStatusRequest {
                session_id: SessionId::new().to_string(),
            })
            .await
            .expect_err("status of an unknown session must error");
        assert_eq!(error.data.unwrap()["error"], SESSION_STATE_NOT_FOUND);

        // Ephemeral agent: even a primed session has no restorable state.
        let (ephemeral_agent, _rx) = build_agent(true).await;
        let primed = primed_parent(&ephemeral_agent, cwd.path());
        let status = ephemeral_agent
            .session_state_status(SessionStateStatusRequest {
                session_id: primed.to_string(),
            })
            .await
            .expect("status");
        assert!(
            !status.saved,
            "ephemeral mode persists no CLI transcript, so nothing is saved"
        );
    }

    /// `session/pin` is a no-op for the claude backend (no cache eviction to
    /// pin against): it succeeds and reports the effective pin state `false`
    /// rather than pretending. An unknown session still errors.
    #[tokio::test]
    #[serial]
    async fn test_pin_is_noop_reporting_effective_state() {
        let _state = StateDirGuard::new();
        let cwd = tempfile::tempdir().unwrap();
        let (agent, _rx) = build_agent(false).await;
        let primed = primed_parent(&agent, cwd.path());

        for pinned in [true, false] {
            let response = agent
                .pin_session(SessionPinRequest {
                    session_id: primed.to_string(),
                    pinned,
                })
                .await
                .expect("pin/unpin must succeed as a no-op");
            assert!(
                !response.pinned,
                "the claude backend has no pinning; effective state is false"
            );
        }

        let error = agent
            .pin_session(SessionPinRequest {
                session_id: SessionId::new().to_string(),
                pinned: true,
            })
            .await
            .expect_err("pin of an unknown session must error");
        assert_eq!(error.data.unwrap()["error"], SESSION_STATE_NOT_FOUND);
    }

    /// The CLI cleanly rejecting the fork (immediate exit: parent transcript
    /// missing or `--fork-session` unsupported) is the genuine "parent state
    /// unavailable" condition — the permanent fallback signal the contract
    /// reserves for it.
    #[test]
    fn test_fork_attach_rejection_maps_to_state_unavailable() {
        let error = fork_attach_error(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            crate::claude::ForkAttachError::Rejected {
                detail: "exit status: 1: No conversation found with session ID".to_string(),
            },
        );

        assert_eq!(error.code, agent_client_protocol::ErrorCode::InvalidParams);
        assert_eq!(
            error.data.expect("error must carry structured data")["error"],
            FORK_PARENT_STATE_UNAVAILABLE
        );
    }

    /// An environment failure launching the forked process (claude binary
    /// missing, spawn I/O error) says nothing about the parent's state: it
    /// must surface as a retryable internal error, NEVER as
    /// `fork_parent_state_unavailable` — that kind tells the client to
    /// re-prime via `session/new`, which would retry futilely against a
    /// broken CLI install.
    #[test]
    fn test_fork_attach_spawn_failure_maps_to_internal_error() {
        let error = fork_attach_error(
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            crate::claude::ForkAttachError::Spawn(crate::AgentError::Internal(
                "claude binary not found in PATH".to_string(),
            )),
        );

        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::InternalError,
            "a spawn failure is a retryable internal error: {error:?}"
        );
        let masquerades = error.data.as_ref().is_some_and(|data| {
            data["error"] == FORK_PARENT_STATE_UNAVAILABLE || data["error"] == FORK_PARENT_NOT_FOUND
        });
        assert!(
            !masquerades,
            "a spawn failure must never masquerade as a parent-state condition: {error:?}"
        );
    }

    /// The fork extension surface is reachable through `ext_method` under the
    /// shared method names — the route a real ACP client takes.
    #[tokio::test]
    #[serial]
    async fn test_ext_method_routes_fork_status_and_pin() {
        let _state = StateDirGuard::new();
        let cwd = tempfile::tempdir().unwrap();
        let (agent, _rx) = build_agent(false).await;
        let parent_id = primed_parent(&agent, cwd.path());

        let call = |method: &'static str, params: serde_json::Value| {
            let agent = &agent;
            async move {
                let raw = RawValue::from_string(params.to_string()).unwrap();
                let response = agent
                    .ext_method(ExtRequest::new(method, Arc::from(raw)))
                    .await
                    .unwrap_or_else(|e| panic!("{method} must route and succeed: {e:?}"));
                serde_json::from_str::<serde_json::Value>(response.0.get()).unwrap()
            }
        };

        let status = call(
            SESSION_STATE_STATUS_METHOD,
            serde_json::json!({"sessionId": parent_id.to_string()}),
        )
        .await;
        assert_eq!(status["saved"], true);
        assert!(status.get("promptTokens").is_none());

        let pinned = call(
            SESSION_PIN_METHOD,
            serde_json::json!({"sessionId": parent_id.to_string(), "pinned": true}),
        )
        .await;
        assert_eq!(pinned["pinned"], false);

        let fork = call(
            SESSION_FORK_METHOD,
            serde_json::json!({"parentSessionId": parent_id.to_string()}),
        )
        .await;
        assert_eq!(fork["stateAttached"], true);
        assert!(fork.get("prefixTokens").is_none());
        assert!(fork["sessionId"].as_str().is_some());
    }
}
