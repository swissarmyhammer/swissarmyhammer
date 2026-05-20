//! Session resume and load support backed by the claude CLI's own `--resume`.
//!
//! This module is the live `session/resume` and `session/load` machinery for
//! claude-agent. Both methods restore a session from a durable
//! [`SessionRecord`](agent_client_protocol_extras::SessionRecord) produced by
//! the shared [`SessionStore`](agent_client_protocol_extras::SessionStore):
//!
//! - `session/resume` restores state and returns — it MUST NOT replay history.
//! - `session/load` restores state, then replays the recorded conversation as
//!   `session/update` notifications, then returns.
//!
//! State restoration is shared by both and is the [`ResumeStrategy::restore`]
//! implementation below: rehydrate the in-memory [`Session`](crate::session::Session)
//! from the record and re-spawn the claude CLI with `--resume <uuid>` so the
//! CLI reattaches to its own transcript. The replay step is the only thing
//! `session/load` does beyond `session/resume`.
//!
//! The validation logic here — record expiration, record integrity, and the
//! replay error-recovery with exponential backoff — was salvaged from the
//! former `session_loading.rs` `EnhancedSessionLoader` / `SessionHistoryReplayer`,
//! which were never wired into the live handlers. Here they run on the real
//! path, against the agent-neutral `SessionRecord` rather than the in-memory
//! `Session`.

use std::time::Duration;

use agent_client_protocol::schema::{
    SessionId as AcpSessionId, SessionNotification, SessionUpdate,
};
use agent_client_protocol_extras::{ResumeStrategy, SessionRecord, SessionStoreError};

use crate::agent::ClaudeAgent;
use crate::session::{Message, Session, SessionId};

/// Maximum age of a persisted [`SessionRecord`] that may still be resumed.
///
/// Records whose `updated_at` is older than this are treated as expired.
/// Salvaged from the former `EnhancedSessionLoader::max_session_age`.
const MAX_RECORD_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Maximum number of `SessionUpdate`s a record may carry before it is treated
/// as corrupt. Salvaged from `EnhancedSessionLoader::max_history_messages`.
const MAX_RECORD_UPDATES: usize = 10_000;

/// Maximum consecutive replay-notification failures tolerated before
/// `session/load` aborts the replay. Salvaged from
/// `SessionHistoryReplayer::max_replay_failures`.
const MAX_REPLAY_FAILURES: usize = 5;

/// Base delay, in milliseconds, used for replay error-recovery backoff.
/// Salvaged from `SessionHistoryReplayer::replay_delay_ms`.
const REPLAY_BACKOFF_BASE_MS: u64 = 10;

/// A reason a [`SessionRecord`] cannot be resumed or loaded.
///
/// These map to ACP `invalid_params` errors at the handler boundary: the
/// session id is opaque and valid, but the persisted record for it cannot be
/// used. This is a lookup/state failure, never a session-id format rejection.
#[derive(Debug, thiserror::Error)]
pub enum SessionRestoreError {
    /// No persisted record exists for the session id.
    #[error("no persisted session record for session {0}")]
    NotFound(String),

    /// The record exists but is older than [`MAX_RECORD_AGE`].
    #[error("session {session_id} expired (last activity {updated_at}, max age {max_age_secs}s)")]
    Expired {
        /// The session id whose record expired.
        session_id: String,
        /// The record's RFC 3339 last-activity timestamp.
        updated_at: String,
        /// The configured maximum record age, in seconds.
        max_age_secs: u64,
    },

    /// The record failed integrity validation.
    #[error("session {session_id} record is corrupt: {detail}")]
    Corrupt {
        /// The session id whose record is corrupt.
        session_id: String,
        /// What about the record failed validation.
        detail: String,
    },

    /// The session id string is not a valid claude-agent session id.
    ///
    /// The id is opaque, but claude-agent's CLI bridge needs to derive a
    /// deterministic UUID from it, which only works for ULID ids. A non-ULID
    /// id surfaces here as a resume failure, never as a format pre-rejection.
    #[error("session id {0} cannot be resumed by the claude backend")]
    UnusableId(String),

    /// The claude CLI could not resume its own transcript for the session.
    #[error("claude CLI could not resume session {session_id}: {detail}")]
    CliResumeFailed {
        /// The session id the CLI failed to resume.
        session_id: String,
        /// The underlying CLI failure detail.
        detail: String,
    },
}

impl ClaudeAgent {
    /// Load and validate the persisted [`SessionRecord`] for an opaque session
    /// id, applying the salvaged expiration and integrity checks.
    ///
    /// This is the shared first half of both `session/resume` and
    /// `session/load`: it resolves the durable record and rejects it if it is
    /// missing, expired, or corrupt. The caller then restores state via
    /// [`ResumeStrategy::restore`].
    ///
    /// # Errors
    ///
    /// Returns [`SessionRestoreError::NotFound`] when no record is persisted,
    /// [`SessionRestoreError::Expired`] when the record is older than
    /// [`MAX_RECORD_AGE`], or [`SessionRestoreError::Corrupt`] when the record
    /// fails integrity validation. A store I/O failure is reported as
    /// `Corrupt` so the handler still fails closed.
    pub(crate) fn load_session_record(
        &self,
        session_id: &str,
    ) -> Result<SessionRecord, SessionRestoreError> {
        let record = agent_client_protocol_extras::SessionStore::new()
            .load(session_id)
            .map_err(|e| SessionRestoreError::Corrupt {
                session_id: session_id.to_string(),
                detail: format!("session store could not be read: {e}"),
            })?
            .ok_or_else(|| SessionRestoreError::NotFound(session_id.to_string()))?;

        Self::check_record_expiration(&record)?;
        Self::check_record_integrity(&record)?;
        Ok(record)
    }

    /// Reject a [`SessionRecord`] whose last activity is older than
    /// [`MAX_RECORD_AGE`]. Salvaged from `EnhancedSessionLoader`'s step 5.
    fn check_record_expiration(record: &SessionRecord) -> Result<(), SessionRestoreError> {
        let updated_at = chrono::DateTime::parse_from_rfc3339(&record.updated_at).map_err(|e| {
            SessionRestoreError::Corrupt {
                session_id: record.session_id.clone(),
                detail: format!("updated_at is not a valid RFC 3339 timestamp: {e}"),
            }
        })?;

        let age = chrono::Utc::now().signed_duration_since(updated_at);
        // A negative age (record timestamped in the future) is an integrity
        // problem, handled by `check_record_integrity`; only a positive age
        // beyond the limit counts as expiry here.
        if age.num_seconds() > 0 && (age.num_seconds() as u64) > MAX_RECORD_AGE.as_secs() {
            return Err(SessionRestoreError::Expired {
                session_id: record.session_id.clone(),
                updated_at: record.updated_at.clone(),
                max_age_secs: MAX_RECORD_AGE.as_secs(),
            });
        }
        Ok(())
    }

    /// Reject a structurally invalid [`SessionRecord`]. Salvaged from
    /// `EnhancedSessionLoader::validate_session_integrity`, adapted to the
    /// agent-neutral record (which has no per-message timestamps).
    fn check_record_integrity(record: &SessionRecord) -> Result<(), SessionRestoreError> {
        let updated_at = chrono::DateTime::parse_from_rfc3339(&record.updated_at).map_err(|e| {
            SessionRestoreError::Corrupt {
                session_id: record.session_id.clone(),
                detail: format!("updated_at is not a valid RFC 3339 timestamp: {e}"),
            }
        })?;

        // A record timestamped in the future indicates clock corruption or a
        // tampered record — fail closed rather than resume from it.
        if updated_at > chrono::Utc::now() {
            return Err(SessionRestoreError::Corrupt {
                session_id: record.session_id.clone(),
                detail: "updated_at timestamp is in the future".to_string(),
            });
        }

        if record.updates.len() > MAX_RECORD_UPDATES {
            return Err(SessionRestoreError::Corrupt {
                session_id: record.session_id.clone(),
                detail: format!(
                    "record carries {} updates, exceeding the maximum of {}",
                    record.updates.len(),
                    MAX_RECORD_UPDATES
                ),
            });
        }
        Ok(())
    }

    /// Rehydrate the in-memory [`Session`](crate::session::Session) for a
    /// resumed session from its durable [`SessionRecord`].
    ///
    /// After a process restart the in-memory `SessionManager` is empty, so the
    /// subsequent `session/prompt` would not find the session. This reconstructs
    /// the live session from the record and inserts it, restoring the cwd, the
    /// MCP server configuration, and the accumulated `SessionUpdate` history.
    ///
    /// Visible crate-wide and to the integration suite so the rehydration step
    /// can be exercised in isolation, without driving a live `claude --resume`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionRestoreError::Corrupt`] if the reconstructed session
    /// cannot be inserted into the in-memory `SessionManager`.
    pub fn rehydrate_in_memory_session(
        &self,
        session_id: SessionId,
        record: &SessionRecord,
    ) -> Result<(), SessionRestoreError> {
        let mut session = Session::new(session_id, record.cwd.clone());
        session.mcp_servers = record
            .mcp_servers
            .iter()
            .map(|server| serde_json::to_string(server).unwrap_or_else(|_| format!("{server:?}")))
            .collect();
        for update in &record.updates {
            session.add_message(Message::from_update(update.clone()));
        }

        self.session_manager.restore_session(session).map_err(|e| {
            SessionRestoreError::Corrupt {
                session_id: record.session_id.clone(),
                detail: format!("could not rehydrate in-memory session: {e}"),
            }
        })?;

        // The transcript recorder is keyed by session ULID and opens
        // `raw.jsonl` in append mode, so re-wiring it for a resumed session
        // keeps appending to the existing transcript rather than truncating it.
        self.wire_raw_message_manager(&session_id);
        Ok(())
    }

    /// Replay a record's `SessionUpdate` stream to the client as
    /// `session/update` notifications, with error-recovery and backoff.
    ///
    /// This is the replay half of `session/load` — the only step that
    /// distinguishes it from `session/resume`. Salvaged from
    /// `SessionHistoryReplayer::replay_history_with_recovery`: a failed update
    /// is logged and skipped — the loop advances to the next update rather than
    /// resending it — and the stream backs off with an exponential delay before
    /// continuing. The replay aborts only after [`MAX_REPLAY_FAILURES`]
    /// consecutive failures.
    ///
    /// Visible crate-wide and to the integration suite so the replay loop —
    /// the most intricate piece of the resume machinery — can be exercised
    /// directly, without driving a live `claude --resume`.
    ///
    /// # Errors
    ///
    /// Returns [`SessionRestoreError::Corrupt`] if the notification channel
    /// fails [`MAX_REPLAY_FAILURES`] times in a row, identifying how far the
    /// replay got.
    pub async fn replay_record_updates(
        &self,
        record: &SessionRecord,
    ) -> Result<(), SessionRestoreError> {
        if record.updates.is_empty() {
            return Ok(());
        }

        let total = record.updates.len();
        tracing::info!(
            "Replaying {} session updates for session {}",
            total,
            record.session_id
        );

        let mut consecutive_failures = 0usize;
        for (index, update) in record.updates.iter().enumerate() {
            let notification = Self::build_replay_notification(record, update, index, total);
            // The error branch below — and therefore the consecutive-failure
            // abort — cannot be reached in tests: `NotificationSender::send_update`
            // is infallible (a broadcast send with no live subscriber is
            // discarded, never surfaced as `Err`), and `notification_sender` is
            // a concrete type, so no failing sender can be injected. The
            // error-recovery path is kept verbatim from the salvaged
            // `SessionHistoryReplayer` to stay correct if `send_update` ever
            // becomes fallible; only the success path is test-covered.
            match self.notification_sender.send_update(notification).await {
                Ok(()) => consecutive_failures = 0,
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::error!(
                        "Failed to replay update {} of {} for session {}: {}",
                        index + 1,
                        total,
                        record.session_id,
                        e
                    );
                    if consecutive_failures >= MAX_REPLAY_FAILURES {
                        return Err(SessionRestoreError::Corrupt {
                            session_id: record.session_id.clone(),
                            detail: format!(
                                "history replay aborted after {consecutive_failures} \
                                 consecutive failures at update {} of {total}",
                                index + 1
                            ),
                        });
                    }
                    // Exponential backoff before retrying the stream.
                    let backoff =
                        Duration::from_millis(REPLAY_BACKOFF_BASE_MS << consecutive_failures);
                    tokio::time::sleep(backoff).await;
                }
            }
        }

        tracing::info!(
            "Completed history replay for session {} ({} updates)",
            record.session_id,
            total
        );
        Ok(())
    }

    /// Map a [`SessionRestoreError`] onto an ACP error for the `session/load`
    /// and `session/resume` handlers.
    ///
    /// Every restore failure is reported as `invalid_params` (-32602): the
    /// session id is a valid opaque string, but the durable record for it is
    /// missing, expired, corrupt, or could not be resumed by the claude CLI.
    /// The session id is **never** rejected on format — an id that is not a
    /// ULID surfaces here as an [`UnusableId`](SessionRestoreError::UnusableId)
    /// resume failure, not a format pre-rejection.
    pub(crate) fn restore_error_to_acp(
        &self,
        session_id: &AcpSessionId,
        error: SessionRestoreError,
    ) -> agent_client_protocol::Error {
        tracing::warn!("Session restore failed for {}: {}", session_id, error);
        let kind = match error {
            SessionRestoreError::NotFound(_) => "session_not_found",
            SessionRestoreError::Expired { .. } => "session_expired",
            SessionRestoreError::Corrupt { .. } => "session_corrupt",
            SessionRestoreError::UnusableId(_) => "session_unusable",
            SessionRestoreError::CliResumeFailed { .. } => "session_resume_failed",
        };
        crate::acp_error::invalid_params(error.to_string()).data(serde_json::json!({
            "sessionId": session_id,
            "error": kind,
        }))
    }

    /// Map the [`SessionStoreError`] produced by [`ResumeStrategy::restore`]
    /// onto an ACP error for the `session/load` and `session/resume` handlers.
    ///
    /// `restore` carries an agent-side failure as a
    /// [`SessionStoreError::Io`]; this surfaces it as `invalid_params` (-32602)
    /// with the underlying detail, consistent with [`restore_error_to_acp`].
    pub(crate) fn session_restore_failed_error(
        &self,
        session_id: &AcpSessionId,
        error: &SessionStoreError,
    ) -> agent_client_protocol::Error {
        tracing::warn!("Session state restore failed for {}: {}", session_id, error);
        crate::acp_error::invalid_params(format!(
            "Session {session_id} could not be restored: {error}"
        ))
        .data(serde_json::json!({
            "sessionId": session_id,
            "error": "session_resume_failed",
        }))
    }

    /// Build one `session/update` notification for a replayed update, tagging
    /// it as a historical replay so clients can distinguish it from live
    /// output.
    fn build_replay_notification(
        record: &SessionRecord,
        update: &SessionUpdate,
        index: usize,
        total: usize,
    ) -> SessionNotification {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "message_type".to_string(),
            serde_json::json!("historical_replay"),
        );
        meta.insert("message_index".to_string(), serde_json::json!(index));
        meta.insert("total_messages".to_string(), serde_json::json!(total));

        SessionNotification::new(AcpSessionId::new(record.session_id.clone()), update.clone())
            .meta(meta)
    }
}

#[async_trait::async_trait]
impl ResumeStrategy for ClaudeAgent {
    /// Restore claude-agent's generation state for a resumed session.
    ///
    /// Restoration is two steps, and is shared verbatim by `session/resume`
    /// and `session/load`:
    ///
    /// 1. Rehydrate the in-memory [`Session`](crate::session::Session) from the
    ///    record so the next `session/prompt` finds the session after a
    ///    process restart.
    /// 2. Re-spawn the claude CLI with `--resume <uuid>` so the CLI reattaches
    ///    to its own transcript for the session's deterministic UUID.
    ///
    /// Per the [`ResumeStrategy`] contract this restores state only — it never
    /// replays history. `session/load`'s replay step is
    /// [`replay_record_updates`](Self::replay_record_updates), invoked by that
    /// handler after `restore` returns.
    ///
    /// # Errors
    ///
    /// Returns a [`SessionStoreError::Io`] wrapping a [`SessionRestoreError`]
    /// when the session id is unusable, the in-memory session cannot be
    /// rehydrated, or the claude CLI has no transcript to resume.
    async fn restore(&self, record: &SessionRecord) -> Result<(), SessionStoreError> {
        let session_id = SessionId::parse(&record.session_id).map_err(|_| {
            restore_io_error(SessionRestoreError::UnusableId(record.session_id.clone()))
        })?;

        // Step 1: rebuild the live in-memory session from the durable record.
        self.rehydrate_in_memory_session(session_id, record)
            .map_err(restore_io_error)?;

        // Step 2: re-spawn the claude CLI in resume mode so it reattaches to
        // its own transcript for this session's deterministic UUID.
        let spawn_config = crate::claude_process::SpawnConfig::builder()
            .session_id(session_id)
            .acp_session_id(AcpSessionId::new(record.session_id.clone()))
            .cwd(record.cwd.clone())
            .mcp_servers(self.config.mcp_servers.clone())
            .ephemeral(self.config.claude.ephemeral)
            .tools_override(self.config.claude.tools_override.clone())
            .resume(true)
            .build();

        self.claude_client
            .resume_process(spawn_config)
            .await
            .map_err(|e| {
                restore_io_error(SessionRestoreError::CliResumeFailed {
                    session_id: record.session_id.clone(),
                    detail: e.to_string(),
                })
            })?;

        tracing::info!("Restored session {} via claude --resume", record.session_id);
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
