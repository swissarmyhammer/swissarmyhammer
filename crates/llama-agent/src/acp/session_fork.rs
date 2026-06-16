//! Session forking over ACP: a new session seeded from a parent session's
//! conversation and cached generation state.
//!
//! This module implements llama-agent's side of the backend-agnostic
//! extension contract in [`agent_client_protocol_extras::session_fork`] —
//! three `ext_method`s routed from [`AcpServer::ext_method`]:
//!
//! - `session/fork` ([`AcpServer::fork_session`]) — clone the parent's llama
//!   [`Session`](crate::types::Session) (history, tools, MCP routing) under a
//!   fresh id and alias the parent's cached KV snapshot to the child with no
//!   byte copy ([`RequestQueue::fork_session_state`](crate::queue::RequestQueue::fork_session_state)).
//! - `session/state_status` ([`AcpServer::session_state_status`]) — report
//!   whether a session's KV snapshot is saved, its prompt-token count, byte
//!   size, and pin state, so a client can confirm a fork will attach state
//!   instead of forking blind.
//! - `session/pin` ([`AcpServer::pin_session`]) — pin/unpin the snapshot
//!   against cache eviction while forks are in flight.
//!
//! # Why forking sidesteps the hybrid-model rollback limit
//!
//! The child's entry in the session-state cache carries the parent's
//! prompt-token fingerprint, and the child's conversation re-renders to a
//! strict byte-extension of the parent's saved prompt (the chat template
//! renders `[history + new user turn]` by appending; the saved tokens end at
//! the parent's generation-prompt boundary). The streaming KV preparation
//! therefore computes `lcp == donor length`, and trimming the restored state
//! to that offset clears an EMPTY range — zero rollback, so the hybrid
//! attention+recurrent models whose `clear_kv_cache_seq` cannot roll back
//! (`Ok(false)` → cold prefill) decode strictly forward from the parent's
//! saved position.

use agent_client_protocol::schema::SessionId as AcpSessionId;
use agent_client_protocol_extras::{
    SessionErrorKind, SessionForkRequest, SessionForkResponse, SessionPinRequest,
    SessionPinResponse, SessionStateStatusRequest, SessionStateStatusResponse,
};

use super::server::AcpServer;
use super::session::AcpSessionState;
use crate::types::ids::SessionId as LlamaSessionId;
use crate::types::Session;

/// Build the structured `invalid_params` error every fork/status/pin failure
/// maps onto: the message is human-readable, and `data` carries the session id
/// plus a machine-readable error kind from the shared extension contract so
/// the client can branch on it.
///
/// `kind` is the typed [`SessionErrorKind`] (not a bare `&str`) so the legal
/// set of `data.error` values stays closed and named, and a swap of the
/// `kind`/`message` arguments is a compile error. This is the same shape as
/// claude-agent's `acp_error::session_error`, keeping the two backends'
/// session errors in lockstep.
fn extension_error(
    session_id: &str,
    kind: SessionErrorKind,
    message: impl std::fmt::Display,
) -> agent_client_protocol::Error {
    tracing::warn!("Session extension call failed for {session_id}: {message}");
    super::acp_error::invalid_params(message.to_string()).data(serde_json::json!({
        "sessionId": session_id,
        "error": kind.as_str(),
    }))
}

impl AcpServer {
    /// Handle the `session/fork` extension: create a new session seeded from
    /// `parent_session_id`'s conversation and cached KV state.
    ///
    /// The parent's state is attached FIRST (it is the cheap, fallible step),
    /// so a parent that cannot seed a fork never half-creates a child session.
    /// On success the child:
    ///
    /// - holds a clone of the parent's conversation, tools, and cwd under a
    ///   fresh llama session id (fresh timestamps, no title);
    /// - shares the parent's MCP client routing (`Arc`-cloned, no reconnect);
    /// - owns a cache entry aliasing the parent's KV snapshot (shared blob,
    ///   counted once) registered with the parent's prompt fingerprint — its
    ///   first prompt restores the full donor state and decodes strictly
    ///   forward with zero rollback;
    /// - is fully independent: its own end-of-turn save inserts its own cache
    ///   entry (copy-on-write), never mutating the parent's.
    ///
    /// # Errors
    ///
    /// `invalid_params` with `data.error` distinguishing the failure:
    /// [`SessionErrorKind::ForkParentNotFound`] when the parent session does not
    /// exist, [`SessionErrorKind::ForkParentStateUnavailable`] when it exists
    /// but has no saved,
    /// strict-prefix-restorable state — the caller falls back to a plain
    /// `session/new` + full prompt, or re-primes. A degraded fork is never
    /// created silently.
    pub async fn fork_session(
        &self,
        request: SessionForkRequest,
    ) -> Result<SessionForkResponse, agent_client_protocol::Error> {
        let (parent_state, parent_session) = self.resolve_fork_parent(&request).await?;

        // Attach the parent's cached KV state to the child id FIRST: this is
        // the fallible gate ("never fork blind"), and failing here leaves no
        // half-created child behind.
        let child_id = LlamaSessionId::new();
        let fork_info = self
            .agent_server()
            .request_queue()
            .fork_session_state(&parent_state.llama_session_id, &child_id)
            .map_err(|e| {
                extension_error(
                    &request.parent_session_id,
                    SessionErrorKind::ForkParentStateUnavailable,
                    e,
                )
            })?;

        let child_session = clone_child_session(&parent_session, child_id);
        if let Err(e) = self
            .agent_server()
            .session_manager()
            .restore_session(child_session.clone())
            .await
        {
            // Roll back the aliased cache entry so the failed fork leaves
            // nothing behind.
            self.agent_server()
                .request_queue()
                .evict_session_state(&child_id);
            return Err(super::acp_error::internal_error(format!(
                "forked session could not be stored: {e}"
            )));
        }

        self.share_parent_mcp_clients(&parent_state.llama_session_id, child_id)
            .await;

        // Register the ACP-layer state, durable record, transcript recorder,
        // and hooks — the identical lifecycle `session/new` runs.
        let child_session_id = self
            .register_session(
                child_id,
                parent_state.client_capabilities.clone(),
                child_session.cwd.clone(),
            )
            .await;

        tracing::info!(
            "Forked session {} from parent {} ({} messages, {} prefix tokens, {} state bytes shared)",
            child_session_id.0,
            request.parent_session_id,
            child_session.messages.len(),
            fork_info.prefix_tokens,
            fork_info.state_bytes
        );

        Ok(SessionForkResponse {
            session_id: child_session_id.0.to_string(),
            state_attached: true,
            prefix_tokens: Some(fork_info.prefix_tokens as u64),
        })
    }

    /// Resolve the fork parent: its ACP-layer state and its live llama
    /// conversation.
    ///
    /// Distinguishes "the parent genuinely does not exist" (the permanent
    /// [`SessionErrorKind::ForkParentNotFound`] fallback signal) from a
    /// session-store
    /// FAILURE (I/O, corrupt record): the latter is a retryable internal
    /// error — misreporting it as not-found would make the client abandon a
    /// parent that is actually alive.
    async fn resolve_fork_parent(
        &self,
        request: &SessionForkRequest,
    ) -> Result<(AcpSessionState, Session), agent_client_protocol::Error> {
        let parent_acp_id = AcpSessionId::new(request.parent_session_id.clone());
        let parent_state = self.get_session(&parent_acp_id).await.ok_or_else(|| {
            extension_error(
                &request.parent_session_id,
                SessionErrorKind::ForkParentNotFound,
                format!(
                    "fork parent session {} not found",
                    request.parent_session_id
                ),
            )
        })?;

        let parent_session = match self
            .agent_server()
            .session_manager()
            .get_session(&parent_state.llama_session_id)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => {
                return Err(extension_error(
                    &request.parent_session_id,
                    SessionErrorKind::ForkParentNotFound,
                    format!(
                        "fork parent session {} has no live conversation state",
                        request.parent_session_id
                    ),
                ));
            }
            Err(e) => {
                tracing::error!(
                    "Session store failed while resolving fork parent {}: {e}",
                    request.parent_session_id
                );
                return Err(super::acp_error::internal_error(format!(
                    "session store failed while resolving fork parent {}: {e}",
                    request.parent_session_id
                )));
            }
        };

        Ok((parent_state, parent_session))
    }

    /// Share the parent's MCP backends and tool-routing index with the child
    /// (Arc clones, no reconnect) so the child's agentic loop dispatches
    /// tools exactly like the parent's. A parent with no MCP clients (never
    /// prompted) is a no-op.
    async fn share_parent_mcp_clients(
        &self,
        parent_llama_id: &LlamaSessionId,
        child_id: LlamaSessionId,
    ) {
        let parent_clients = {
            let clients = self.agent_server().session_mcp_clients.read().await;
            clients.get(parent_llama_id).cloned()
        };
        if let Some(clients) = parent_clients {
            self.agent_server()
                .session_mcp_clients
                .write()
                .await
                .insert(child_id, clients);
        }
    }
}

/// Clone the parent's conversation under the fresh child id: fresh
/// timestamps, no title. The clone keeps `cached_message_count` /
/// `cached_token_count` — they describe the parent's saved state, which the
/// child's cache entry now aliases.
fn clone_child_session(parent: &Session, child_id: LlamaSessionId) -> Session {
    let now = std::time::SystemTime::now();
    let mut child = parent.clone();
    child.id = child_id;
    child.created_at = now;
    child.updated_at = now;
    child.title = None;
    child
}

impl AcpServer {
    /// Handle the `session/state_status` extension: report whether a session's
    /// KV snapshot is saved and restorable, so a client can confirm before
    /// forking ("never fork blind").
    ///
    /// `saved: true` with `prompt_tokens: Some(..)` means the snapshot can
    /// seed a strict-prefix fork; a session with no snapshot reports
    /// `saved: false` (not an error — "not saved" is a valid answer).
    ///
    /// # Errors
    ///
    /// `invalid_params` when the session itself does not exist.
    pub async fn session_state_status(
        &self,
        request: SessionStateStatusRequest,
    ) -> Result<SessionStateStatusResponse, agent_client_protocol::Error> {
        let acp_id = AcpSessionId::new(request.session_id.clone());
        let state = self.get_session(&acp_id).await.ok_or_else(|| {
            extension_error(
                &request.session_id,
                SessionErrorKind::SessionStateNotFound,
                format!("session {} not found", request.session_id),
            )
        })?;

        Ok(
            match self
                .agent_server()
                .request_queue()
                .session_state_status(&state.llama_session_id)
            {
                Some(status) => SessionStateStatusResponse {
                    saved: true,
                    prompt_tokens: status.prompt_tokens.map(|t| t as u64),
                    bytes: Some(status.state_bytes as u64),
                    pinned: status.pinned,
                },
                None => SessionStateStatusResponse {
                    saved: false,
                    prompt_tokens: None,
                    bytes: None,
                    pinned: false,
                },
            },
        )
    }

    /// Handle the `session/pin` extension: pin (or unpin) a session's KV
    /// snapshot against cache eviction.
    ///
    /// Pinning a session with no saved snapshot is an error — the pin cannot
    /// take effect and the client must know rather than trust a phantom pin.
    /// Unpinning a session with no snapshot is a benign no-op (`pinned:
    /// false`): the entry the client wanted released is already gone.
    ///
    /// # Errors
    ///
    /// `invalid_params` with `data.error == `[`SessionErrorKind::SessionStateNotFound`]
    /// when the session does not exist, or a pin was requested and no snapshot
    /// is cached.
    pub async fn pin_session(
        &self,
        request: SessionPinRequest,
    ) -> Result<SessionPinResponse, agent_client_protocol::Error> {
        let acp_id = AcpSessionId::new(request.session_id.clone());
        let state = self.get_session(&acp_id).await.ok_or_else(|| {
            extension_error(
                &request.session_id,
                SessionErrorKind::SessionStateNotFound,
                format!("session {} not found", request.session_id),
            )
        })?;

        let updated = self
            .agent_server()
            .request_queue()
            .pin_session_state(&state.llama_session_id, request.pinned);
        if !updated && request.pinned {
            return Err(extension_error(
                &request.session_id,
                SessionErrorKind::SessionStateNotFound,
                format!("session {} has no saved state to pin", request.session_id),
            ));
        }

        Ok(SessionPinResponse {
            pinned: updated && request.pinned,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use agent_client_protocol::schema::NewSessionRequest;
    use agent_client_protocol_extras::{
        FORK_PARENT_NOT_FOUND, FORK_PARENT_STATE_UNAVAILABLE, SESSION_STATE_NOT_FOUND,
    };
    use serial_test::serial;
    use tempfile::TempDir;

    use super::*;
    use crate::acp::test_utils::{
        create_acp_server_without_model, test_agent_config, test_cwd, StateDirGuard,
    };
    use crate::types::SessionConfig;

    /// Byte size of the KV blob seeded into a primed parent — named so the
    /// `state_status` assertion on `bytes` stays linked to the seed.
    const SEEDED_STATE_BYTES: usize = 64;

    /// Build a model-free `AcpServer` via the shared test wiring. Session
    /// creation and the fork/status/pin extension surface work without a
    /// model.
    async fn build_server() -> Arc<AcpServer> {
        build_server_with_session_config(SessionConfig::default()).await
    }

    /// [`build_server`] with a custom `SessionConfig` (e.g. persistence
    /// enabled so a test can exercise the storage slow path).
    async fn build_server_with_session_config(session_config: SessionConfig) -> Arc<AcpServer> {
        let (server, _rx) = create_acp_server_without_model(test_agent_config(session_config))
            .await
            .expect("model-free ACP server");
        Arc::new(server)
    }

    /// Create a fresh session with the shared test cwd.
    async fn new_session(server: &AcpServer) -> AcpSessionId {
        server
            .new_session(NewSessionRequest::new(test_cwd()))
            .await
            .expect("new_session")
            .session_id
    }

    /// Create a session, add a user/assistant exchange, and seed its cached
    /// KV state — a model-free stand-in for a "primed" parent session.
    async fn primed_parent(server: &AcpServer, prompt_tokens: Vec<i32>) -> AcpSessionId {
        let session_id = new_session(server).await;

        let llama_id: LlamaSessionId = session_id.0.parse().expect("llama ULID");
        for (role, text) in [
            (crate::types::MessageRole::User, "the corpus under review"),
            (crate::types::MessageRole::Assistant, "understood"),
        ] {
            server
                .agent_server()
                .session_manager()
                .add_message(
                    &llama_id,
                    crate::types::Message {
                        role,
                        content: text.to_string(),
                        tool_call_id: None,
                        tool_name: None,
                        timestamp: std::time::SystemTime::now(),
                    },
                )
                .await
                .expect("add_message");
        }

        server
            .agent_server()
            .request_queue()
            .seed_session_state_for_test(
                &llama_id,
                vec![0xAB; SEEDED_STATE_BYTES],
                Some(prompt_tokens),
            );
        session_id
    }

    /// A fork of a primed parent clones the conversation under a fresh id and
    /// reports the attached state's prefix token count.
    #[tokio::test]
    #[serial]
    async fn fork_clones_history_and_attaches_state() {
        let _state = StateDirGuard::new();
        let server = build_server().await;
        let parent_id = primed_parent(&server, vec![10, 20, 30]).await;

        let response = server
            .fork_session(SessionForkRequest {
                parent_session_id: parent_id.0.to_string(),
            })
            .await
            .expect("fork of a primed parent must succeed");

        assert_ne!(response.session_id, parent_id.0.as_ref());
        assert!(response.state_attached, "fork must report attached state");
        assert_eq!(response.prefix_tokens, Some(3));

        // The child session holds the parent's conversation under its own id.
        let child_llama: LlamaSessionId = response.session_id.parse().expect("llama ULID");
        let child = server
            .agent_server()
            .session_manager()
            .get_session(&child_llama)
            .await
            .expect("lookup")
            .expect("forked session must exist");
        assert_eq!(child.messages.len(), 2);
        assert_eq!(child.messages[0].content, "the corpus under review");

        // The child's cache entry aliases the parent's snapshot (same token
        // count) and the parent's entry is intact.
        let child_status = server
            .agent_server()
            .request_queue()
            .session_state_status(&child_llama)
            .expect("child must have an aliased cache entry");
        assert_eq!(child_status.prompt_tokens, Some(3));

        let parent_llama: LlamaSessionId = parent_id.0.parse().unwrap();
        let parent_status = server
            .agent_server()
            .request_queue()
            .session_state_status(&parent_llama)
            .expect("parent entry must survive the fork");
        assert_eq!(parent_status.prompt_tokens, Some(3));

        // Both sessions resolve at the ACP layer.
        assert!(server
            .get_session(&AcpSessionId::new(response.session_id.clone()))
            .await
            .is_some());
        assert!(server.get_session(&parent_id).await.is_some());
    }

    /// Forking an unknown parent fails with the distinguishable
    /// `fork_parent_not_found` error kind.
    #[tokio::test]
    #[serial]
    async fn fork_unknown_parent_errors_distinguishably() {
        let _state = StateDirGuard::new();
        let server = build_server().await;

        let error = server
            .fork_session(SessionForkRequest {
                parent_session_id: LlamaSessionId::new().to_string(),
            })
            .await
            .expect_err("fork of an unknown parent must error");

        let data = error.data.expect("error must carry structured data");
        assert_eq!(data["error"], FORK_PARENT_NOT_FOUND);
    }

    /// Forking a parent that exists but has no saved state fails with the
    /// distinguishable `fork_parent_state_unavailable` error kind — never a
    /// silent degraded fork.
    #[tokio::test]
    #[serial]
    async fn fork_unsaved_parent_errors_distinguishably() {
        let _state = StateDirGuard::new();
        let server = build_server().await;

        let parent_id = new_session(&server).await;

        let error = server
            .fork_session(SessionForkRequest {
                parent_session_id: parent_id.0.to_string(),
            })
            .await
            .expect_err("fork of an unprimed parent must error");

        let data = error.data.expect("error must carry structured data");
        assert_eq!(data["error"], FORK_PARENT_STATE_UNAVAILABLE);

        // The failed fork must leave no half-created child session behind in
        // the live ACP map beyond the parent itself.
        assert!(server.get_session(&parent_id).await.is_some());
    }

    /// A real session-store failure while resolving the fork parent must
    /// surface as an internal error (the client may retry), NEVER as
    /// `fork_parent_not_found` — that kind means "the parent does not exist"
    /// and tells the client to permanently fall back to `session/new`,
    /// abandoning a parent that is actually alive.
    #[tokio::test]
    #[serial]
    async fn fork_store_error_is_internal_error_not_parent_not_found() {
        let _state = StateDirGuard::new();
        let storage_dir = TempDir::new().unwrap();
        let server = build_server_with_session_config(SessionConfig {
            persistence_enabled: true,
            session_storage_dir: Some(storage_dir.path().to_path_buf()),
            ..SessionConfig::default()
        })
        .await;

        let parent_id = new_session(&server).await;
        let llama_id: LlamaSessionId = parent_id.0.parse().expect("llama ULID");

        // Evict the live session, then plant a corrupt session file where the
        // storage slow path will reload it from: the next `get_session` is a
        // real store ERROR (parse failure), not a clean "missing".
        server
            .agent_server()
            .session_manager()
            .delete_session(&llama_id)
            .await
            .expect("delete_session");
        std::fs::write(
            storage_dir.path().join(format!("{llama_id}.json")),
            "{ this is not a session",
        )
        .expect("plant corrupt session file");

        let error = server
            .fork_session(SessionForkRequest {
                parent_session_id: parent_id.0.to_string(),
            })
            .await
            .expect_err("fork must fail when the session store errors");

        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::InternalError,
            "a store failure is a retryable internal error: {error:?}"
        );
        let reported_not_found = error
            .data
            .as_ref()
            .is_some_and(|data| data["error"] == FORK_PARENT_NOT_FOUND);
        assert!(
            !reported_not_found,
            "a store failure must never masquerade as fork_parent_not_found: {error:?}"
        );
    }

    /// A parent that is registered at the ACP layer but whose llama-level
    /// conversation is genuinely gone (clean `Ok(None)`, no store error)
    /// still reports `fork_parent_not_found` — the permanent-fallback kind.
    #[tokio::test]
    #[serial]
    async fn fork_missing_live_session_is_parent_not_found() {
        let _state = StateDirGuard::new();
        let server = build_server().await;
        let parent_id = new_session(&server).await;
        let llama_id: LlamaSessionId = parent_id.0.parse().expect("llama ULID");

        // Persistence is disabled, so deleting the in-memory session makes
        // the next lookup a clean `Ok(None)`.
        server
            .agent_server()
            .session_manager()
            .delete_session(&llama_id)
            .await
            .expect("delete_session");

        let error = server
            .fork_session(SessionForkRequest {
                parent_session_id: parent_id.0.to_string(),
            })
            .await
            .expect_err("fork of a vanished parent must error");

        let data = error.data.expect("error must carry structured data");
        assert_eq!(data["error"], FORK_PARENT_NOT_FOUND);
    }

    /// `session/state_status` reports saved/pinned/token-count truthfully for
    /// primed, unprimed, and pinned sessions.
    #[tokio::test]
    #[serial]
    async fn state_status_reports_truthfully() {
        let _state = StateDirGuard::new();
        let server = build_server().await;

        // Unprimed session: saved == false, not an error.
        let unprimed = new_session(&server).await;
        let status = server
            .session_state_status(SessionStateStatusRequest {
                session_id: unprimed.0.to_string(),
            })
            .await
            .expect("status of an unprimed session is a valid answer");
        assert!(!status.saved);
        assert_eq!(status.prompt_tokens, None);
        assert!(!status.pinned);

        // Primed session: saved == true with the real token count and bytes.
        let primed = primed_parent(&server, vec![1, 2, 3, 4]).await;
        let status = server
            .session_state_status(SessionStateStatusRequest {
                session_id: primed.0.to_string(),
            })
            .await
            .expect("status");
        assert!(status.saved);
        assert_eq!(status.prompt_tokens, Some(4));
        assert_eq!(status.bytes, Some(SEEDED_STATE_BYTES as u64));
        assert!(!status.pinned);

        // Pin → status reflects it; unpin → cleared.
        let pin = server
            .pin_session(SessionPinRequest {
                session_id: primed.0.to_string(),
                pinned: true,
            })
            .await
            .expect("pin of a primed session succeeds");
        assert!(pin.pinned);
        assert!(
            server
                .session_state_status(SessionStateStatusRequest {
                    session_id: primed.0.to_string(),
                })
                .await
                .unwrap()
                .pinned
        );

        let unpin = server
            .pin_session(SessionPinRequest {
                session_id: primed.0.to_string(),
                pinned: false,
            })
            .await
            .expect("unpin succeeds");
        assert!(!unpin.pinned);

        // Unknown session id → error.
        let error = server
            .session_state_status(SessionStateStatusRequest {
                session_id: LlamaSessionId::new().to_string(),
            })
            .await
            .expect_err("status of an unknown session must error");
        assert_eq!(error.data.unwrap()["error"], SESSION_STATE_NOT_FOUND);
    }

    /// Pinning a session with no saved state errors distinguishably (a pin
    /// that cannot take effect must be visible); unpinning the same session
    /// is a benign no-op.
    #[tokio::test]
    #[serial]
    async fn pin_without_saved_state_errors_unpin_is_noop() {
        let _state = StateDirGuard::new();
        let server = build_server().await;
        let session_id = new_session(&server).await;

        let error = server
            .pin_session(SessionPinRequest {
                session_id: session_id.0.to_string(),
                pinned: true,
            })
            .await
            .expect_err("pinning an unprimed session must error");
        assert_eq!(error.data.unwrap()["error"], SESSION_STATE_NOT_FOUND);

        let unpin = server
            .pin_session(SessionPinRequest {
                session_id: session_id.0.to_string(),
                pinned: false,
            })
            .await
            .expect("unpinning an unprimed session is a no-op");
        assert!(!unpin.pinned);
    }

    /// The fork extension surface is reachable through `ext_method` under the
    /// shared method names — the route a real ACP client takes.
    #[tokio::test]
    #[serial]
    async fn ext_method_routes_fork_status_and_pin() {
        use agent_client_protocol::schema::{ExtRequest, RawValue};
        use agent_client_protocol_extras::{
            SESSION_FORK_METHOD, SESSION_PIN_METHOD, SESSION_STATE_STATUS_METHOD,
        };

        let _state = StateDirGuard::new();
        let server = build_server().await;
        let parent_id = primed_parent(&server, vec![7, 8]).await;

        let call = |method: &'static str, params: serde_json::Value| {
            let server = Arc::clone(&server);
            async move {
                let raw = RawValue::from_string(params.to_string()).unwrap();
                let response = server
                    .ext_method(ExtRequest::new(method, Arc::from(raw)))
                    .await
                    .unwrap_or_else(|e| panic!("{method} must route and succeed: {e:?}"));
                serde_json::from_str::<serde_json::Value>(response.0.get()).unwrap()
            }
        };

        let status = call(
            SESSION_STATE_STATUS_METHOD,
            serde_json::json!({"sessionId": parent_id.0.as_ref()}),
        )
        .await;
        assert_eq!(status["saved"], true);
        assert_eq!(status["promptTokens"], 2);

        let pinned = call(
            SESSION_PIN_METHOD,
            serde_json::json!({"sessionId": parent_id.0.as_ref(), "pinned": true}),
        )
        .await;
        assert_eq!(pinned["pinned"], true);

        let fork = call(
            SESSION_FORK_METHOD,
            serde_json::json!({"parentSessionId": parent_id.0.as_ref()}),
        )
        .await;
        assert_eq!(fork["stateAttached"], true);
        assert_eq!(fork["prefixTokens"], 2);
        assert!(fork["sessionId"].as_str().is_some());
    }
}
