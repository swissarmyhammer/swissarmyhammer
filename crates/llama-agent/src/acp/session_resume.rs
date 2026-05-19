//! Session resume and load support backed by chat-template re-rendering.
//!
//! This module is the live `session/resume` and `session/load` machinery for
//! llama-agent. Both methods restore a session from a durable
//! [`SessionRecord`](agent_client_protocol_extras::SessionRecord) produced by
//! the shared [`SessionStore`](agent_client_protocol_extras::SessionStore):
//!
//! - `session/resume` restores state and returns — it MUST NOT replay history.
//! - `session/load` restores state, then replays the recorded conversation as
//!   `session/update` notifications, then returns.
//!
//! State restoration is shared by both and is the [`ResumeStrategy::restore`]
//! implementation below. Unlike claude-agent — which shells out to
//! `claude --resume` and leaves the transcript in an external process — the
//! llama backend holds *all* conversation state itself. The
//! [`SessionRecord`](agent_client_protocol_extras::SessionRecord) **is** the
//! resume input: `restore` reconstructs the in-memory llama
//! [`Session`](crate::types::Session) from the record and re-renders it through
//! the model's chat template ([`chat_template`](crate::chat_template)) so the
//! conversation is primed and the next `session/prompt` continues it.
//!
//! The replay step is the only thing `session/load` does beyond
//! `session/resume`; it lives in the `load_session` handler, not here — this
//! module only restores state.

use std::time::{SystemTime, UNIX_EPOCH};

use agent_client_protocol::schema::SessionId as AcpSessionId;
use agent_client_protocol_extras::{
    ResumeStrategy, SessionRecord, SessionStore, SessionStoreError,
};

use super::server::AcpServer;
use super::session_record::session_updates_to_messages;
use crate::types::ids::SessionId as LlamaSessionId;
use crate::types::Session;

/// A reason a persisted [`SessionRecord`] cannot be resumed or loaded.
///
/// These map to ACP `invalid_params` errors at the handler boundary: the
/// session id is opaque and valid, but the persisted record for it cannot be
/// used. This is a lookup/restore failure, never a session-id format rejection.
#[derive(Debug, thiserror::Error)]
pub enum SessionRestoreError {
    /// No persisted record exists for the session id.
    #[error("no persisted session record for session {0}")]
    NotFound(String),

    /// The session store could not be read.
    #[error("session store could not be read for session {session_id}: {detail}")]
    StoreUnreadable {
        /// The session id whose record could not be loaded.
        session_id: String,
        /// The underlying store failure detail.
        detail: String,
    },

    /// The agent's generation state could not be restored from the record.
    #[error("session {session_id} state could not be restored: {detail}")]
    RestoreFailed {
        /// The session id whose state failed to restore.
        session_id: String,
        /// What about the restore failed.
        detail: String,
    },
}

impl AcpServer {
    /// Load the persisted [`SessionRecord`] for an opaque session id from the
    /// shared [`SessionStore`].
    ///
    /// This is the shared first half of both `session/resume` and
    /// `session/load`: it resolves the durable record so the caller can restore
    /// state from it via [`ResumeStrategy::restore`].
    ///
    /// # Errors
    ///
    /// Returns [`SessionRestoreError::NotFound`] when no record is persisted
    /// for the id, or [`SessionRestoreError::StoreUnreadable`] when the store
    /// directory cannot be scanned or the `session.json` is malformed.
    pub(crate) fn load_session_record(
        &self,
        session_id: &str,
    ) -> Result<SessionRecord, SessionRestoreError> {
        SessionStore::new()
            .load(session_id)
            .map_err(|e| SessionRestoreError::StoreUnreadable {
                session_id: session_id.to_string(),
                detail: e.to_string(),
            })?
            .ok_or_else(|| SessionRestoreError::NotFound(session_id.to_string()))
    }

    /// Map a [`SessionRestoreError`] onto an ACP error for the `session/load`
    /// and `session/resume` handlers.
    ///
    /// Every restore failure is reported as `invalid_params` (-32602): the
    /// session id is a valid opaque string, but the durable record for it is
    /// missing, unreadable, or could not be restored. The session id is
    /// **never** rejected on format.
    pub(crate) fn restore_error_to_acp(
        &self,
        session_id: &AcpSessionId,
        error: SessionRestoreError,
    ) -> agent_client_protocol::Error {
        tracing::warn!("Session restore failed for {}: {}", session_id.0, error);
        let kind = match error {
            SessionRestoreError::NotFound(_) => "session_not_found",
            SessionRestoreError::StoreUnreadable { .. } => "session_store_unreadable",
            SessionRestoreError::RestoreFailed { .. } => "session_restore_failed",
        };
        super::acp_error::invalid_params(error.to_string()).data(serde_json::json!({
            "sessionId": session_id.0,
            "error": kind,
        }))
    }

    /// Map the [`SessionStoreError`] produced by [`ResumeStrategy::restore`]
    /// onto an ACP error for the `session/load` and `session/resume` handlers.
    ///
    /// `restore` carries an agent-side failure as a [`SessionStoreError::Io`];
    /// this surfaces it as `invalid_params` (-32602) with the underlying
    /// detail, consistent with [`restore_error_to_acp`](Self::restore_error_to_acp).
    pub(crate) fn session_restore_failed_error(
        &self,
        session_id: &AcpSessionId,
        error: &SessionStoreError,
    ) -> agent_client_protocol::Error {
        tracing::warn!(
            "Session state restore failed for {}: {}",
            session_id.0,
            error
        );
        super::acp_error::invalid_params(format!(
            "Session {} could not be restored: {error}",
            session_id.0
        ))
        .data(serde_json::json!({
            "sessionId": session_id.0,
            "error": "session_restore_failed",
        }))
    }
}

/// Reconstruct a live llama [`Session`] from a durable [`SessionRecord`].
///
/// The record's `updates` stream is converted back to llama
/// [`Message`](crate::types::Message)s via
/// [`session_updates_to_messages`](super::session_record::session_updates_to_messages),
/// and the session id and working directory are taken straight from the
/// record. The session id is parsed as a llama ULID — llama always mints ULID
/// ids, so a record this agent produced parses cleanly; a non-ULID id (from a
/// foreign record) surfaces as a [`SessionRestoreError::RestoreFailed`] rather
/// than a session-id format rejection.
///
/// The system prompt is intentionally not reconstructed: system messages are
/// agent-internal scaffolding and are deliberately excluded from the persisted
/// `updates` stream (see [`session_record`](super::session_record)). The
/// restored conversation is exactly the client-visible turns the record holds.
fn session_from_record(record: &SessionRecord) -> Result<Session, SessionRestoreError> {
    let session_id = record.session_id.parse::<LlamaSessionId>().map_err(|e| {
        SessionRestoreError::RestoreFailed {
            session_id: record.session_id.clone(),
            detail: format!("session id is not a llama session id: {e}"),
        }
    })?;

    let messages = session_updates_to_messages(&record.updates);
    let updated_at = parse_rfc3339_to_system_time(&record.updated_at);

    Ok(Session {
        id: session_id,
        messages,
        cwd: record.cwd.clone(),
        mcp_servers: Vec::new(),
        available_tools: Vec::new(),
        available_prompts: Vec::new(),
        created_at: updated_at,
        updated_at,
        compaction_history: Vec::new(),
        transcript_path: None,
        context_state: None,
        available_commands: Vec::new(),
        current_mode: None,
        client_capabilities: None,
        cached_message_count: 0,
        cached_token_count: 0,
        title: None,
    })
}

/// Parse an RFC 3339 timestamp into a [`SystemTime`], falling back to the
/// current time when the record's timestamp cannot be parsed.
///
/// A restored session is never blocked by a malformed timestamp — the
/// conversation content is what matters; the timestamp is bookkeeping.
fn parse_rfc3339_to_system_time(timestamp: &str) -> SystemTime {
    match chrono::DateTime::parse_from_rfc3339(timestamp) {
        Ok(dt) => {
            let secs = dt.timestamp();
            if secs >= 0 {
                UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
            } else {
                UNIX_EPOCH
            }
        }
        Err(e) => {
            tracing::warn!(
                "Session record timestamp {:?} is not RFC 3339 ({}); using current time",
                timestamp,
                e
            );
            SystemTime::now()
        }
    }
}

#[async_trait::async_trait]
impl ResumeStrategy for AcpServer {
    /// Restore llama-agent's generation state for a resumed session.
    ///
    /// Restoration is two steps, and is shared verbatim by `session/resume`
    /// and `session/load`:
    ///
    /// 1. Reconstruct the in-memory llama [`Session`](crate::types::Session)
    ///    from the record's `updates` stream and insert it into the session
    ///    manager, so the next `session/prompt` finds the session after a
    ///    process restart.
    /// 2. Re-render the restored conversation through the model's chat template
    ///    so the model is primed to continue it. The record **is** the resume
    ///    input — there is no external process holding state — so re-rendering
    ///    the reconstructed conversation is what "resume" means for this
    ///    backend.
    ///
    /// Per the [`ResumeStrategy`] contract this restores state only — it never
    /// replays history. `session/load`'s replay step is the caller's
    /// responsibility, invoked after `restore` returns.
    ///
    /// The chat-template re-render is best-effort: when no model is loaded the
    /// render is skipped with a warning rather than failing the restore. The
    /// in-memory session reconstruction is the load-bearing step, and the live
    /// `session/prompt` path re-renders the session on every turn regardless.
    ///
    /// # Errors
    ///
    /// Returns a [`SessionStoreError::Io`] wrapping a [`SessionRestoreError`]
    /// when the record's session id is not a llama session id, or the
    /// reconstructed session cannot be inserted into the session manager.
    async fn restore(&self, record: &SessionRecord) -> Result<(), SessionStoreError> {
        // Step 1: rebuild the live in-memory session from the durable record.
        let session = session_from_record(record).map_err(restore_io_error)?;
        let message_count = session.messages.len();

        self.agent_server()
            .session_manager()
            .restore_session(session.clone())
            .await
            .map_err(|e| {
                restore_io_error(SessionRestoreError::RestoreFailed {
                    session_id: record.session_id.clone(),
                    detail: format!("could not restore in-memory session: {e}"),
                })
            })?;

        // Step 2: re-render the restored conversation through the model's chat
        // template so the model is primed to continue it. The render is
        // best-effort — a missing model is logged, not fatal.
        match self.agent_server().render_session_prompt(&session).await {
            Ok(prompt) => tracing::info!(
                "Restored session {} ({} messages); re-rendered prompt is {} chars",
                record.session_id,
                message_count,
                prompt.len()
            ),
            Err(e) => tracing::warn!(
                "Restored session {} ({} messages) but could not re-render the \
                 chat-template prompt ({}); the next prompt turn will render it",
                record.session_id,
                message_count,
                e
            ),
        }

        Ok(())
    }
}

/// Wrap a [`SessionRestoreError`] as the [`SessionStoreError::Io`] variant
/// required by the [`ResumeStrategy::restore`] return type.
///
/// `SessionStoreError` has no agent-specific variant, so an agent-side restore
/// failure is carried as an [`std::io::Error`] whose message preserves the
/// original [`SessionRestoreError`] detail for the handler to surface.
fn restore_io_error(error: SessionRestoreError) -> SessionStoreError {
    SessionStoreError::Io(std::io::Error::other(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MessageRole;
    use agent_client_protocol::schema::{ContentBlock, ContentChunk, SessionUpdate, TextContent};
    use std::path::PathBuf;

    /// Build a `SessionRecord` with a user/assistant exchange for the given id.
    fn record_with_exchange(id: &str) -> SessionRecord {
        let mut record =
            SessionRecord::new(id, PathBuf::from("/work/project"), "2026-05-18T12:00:00Z");
        record.updates = vec![
            SessionUpdate::UserMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("what is 2 + 2?".to_string()),
            ))),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("2 + 2 is 4".to_string()),
            ))),
        ];
        record
    }

    /// `session_from_record` reconstructs the conversation, id, and cwd from a
    /// llama-produced (ULID-keyed) record.
    #[test]
    fn session_from_record_reconstructs_conversation() {
        let id = LlamaSessionId::new().to_string();
        let record = record_with_exchange(&id);

        let session = session_from_record(&record).expect("ULID-keyed record should restore");

        assert_eq!(session.id.to_string(), id);
        assert_eq!(session.cwd, PathBuf::from("/work/project"));
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(session.messages[0].content, "what is 2 + 2?");
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].content, "2 + 2 is 4");
    }

    /// A record whose session id is not a llama ULID surfaces as a restore
    /// failure, not a session-id format rejection — the id stays opaque.
    #[test]
    fn session_from_record_rejects_non_ulid_id() {
        let record = record_with_exchange("not-a-valid-ulid");
        let error = session_from_record(&record).expect_err("non-ULID id cannot be a llama id");
        assert!(matches!(error, SessionRestoreError::RestoreFailed { .. }));
    }

    /// A record with no updates restores to an empty conversation rather than
    /// failing.
    #[test]
    fn session_from_record_allows_empty_conversation() {
        let id = LlamaSessionId::new().to_string();
        let record = SessionRecord::new(&id, PathBuf::from("/work/empty"), "2026-05-18T12:00:00Z");

        let session = session_from_record(&record).expect("empty record should restore");
        assert!(session.messages.is_empty());
    }

    /// A malformed timestamp does not block restoration — it falls back to the
    /// current time so the conversation content still restores.
    #[test]
    fn session_from_record_tolerates_bad_timestamp() {
        let id = LlamaSessionId::new().to_string();
        let mut record = record_with_exchange(&id);
        record.updated_at = "not-a-timestamp".to_string();

        let session = session_from_record(&record).expect("bad timestamp must not block restore");
        assert_eq!(session.messages.len(), 2);
    }

    /// An RFC 3339 timestamp parses to the corresponding `SystemTime`.
    #[test]
    fn parse_rfc3339_round_trips_a_known_instant() {
        let parsed = parse_rfc3339_to_system_time("1970-01-01T00:00:01Z");
        assert_eq!(
            parsed.duration_since(UNIX_EPOCH).unwrap(),
            std::time::Duration::from_secs(1)
        );
    }
}
