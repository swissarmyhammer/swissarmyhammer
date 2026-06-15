//! Backend-agnostic extension contract for forking an ACP session from a
//! parent session's saved state.
//!
//! ACP has no first-class "fork" method, so agents expose forking through the
//! protocol's extension mechanism (`ext_method`). This module is the shared
//! wire contract both ends speak — the validators-pool client on one side and
//! the agents (llama-agent today, claude-agent next) on the other — so the
//! request/response shapes live here rather than being duplicated per agent.
//!
//! Three extension methods make up the contract:
//!
//! - [`SESSION_FORK_METHOD`] (`session/fork`) — create a new session seeded
//!   from a parent session's conversation and (when the backend supports it)
//!   the parent's cached generation state, so the fork's first prompt decodes
//!   strictly forward from the parent's saved position.
//! - [`SESSION_STATE_STATUS_METHOD`] (`session/state_status`) — report whether
//!   a session's generation state is saved and restorable, so a client can
//!   confirm a fork will actually attach state ("never fork blind").
//! - [`SESSION_PIN_METHOD`] (`session/pin`) — pin a session's saved state so
//!   backend cache eviction cannot drop it while forks are in flight; unpin
//!   when done.
//!
//! The types are deliberately backend-agnostic: token counts are optional (a
//! backend that does not track prompt tokens omits them) and pinning is allowed
//! to be a no-op (the response reports the *effective* pin state, which a
//! backend without pinning reports as `false`).

use serde::{Deserialize, Serialize};

/// Extension method name for forking a session from a parent.
pub const SESSION_FORK_METHOD: &str = "session/fork";

/// Extension method name for querying a session's saved-state status.
pub const SESSION_STATE_STATUS_METHOD: &str = "session/state_status";

/// Extension method name for pinning/unpinning a session's saved state.
pub const SESSION_PIN_METHOD: &str = "session/pin";

/// Error kind (carried in the ACP error's `data.error` field) when the fork
/// parent session does not exist.
pub const FORK_PARENT_NOT_FOUND: &str = "fork_parent_not_found";

/// Error kind (carried in the ACP error's `data.error` field) when the fork
/// parent exists but has no saved, restorable state to seed the fork with.
/// The client falls back to a plain `session/new` + full prompt, or re-primes.
pub const FORK_PARENT_STATE_UNAVAILABLE: &str = "fork_parent_state_unavailable";

/// Error kind (carried in the ACP error's `data.error` field) when a pin
/// request targets a session with no saved state — the pin cannot take effect,
/// and the client must know that rather than trust a phantom pin.
pub const SESSION_STATE_NOT_FOUND: &str = "session_state_not_found";

/// Error kind (carried in the ACP error's `data.error` field) when a
/// `sessionId`-accepting method resolves its id to no live session. Not
/// fork-specific — every session-scoped method shares this kind — but it lives
/// alongside the fork kinds because both agents build all of them through the
/// same uniform `{sessionId, error}` error shape.
pub const SESSION_NOT_FOUND: &str = "session_not_found";

/// The machine-readable error kind carried in a session-scoped ACP error's
/// `data.error` field.
///
/// Both agents (`llama-agent`, `claude-agent`) build their session errors
/// through one uniform `{sessionId, error: <kind>}` payload. Passing the kind
/// as this enum instead of a bare `&str` keeps the set of legal kinds closed
/// and named at every call site, so a typo or a positional argument swap is a
/// compile error rather than a silently wrong wire value. Each variant maps to
/// exactly one of the contract's kind constants via [`SessionErrorKind::as_str`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionErrorKind {
    /// The fork parent session does not exist. Maps to [`FORK_PARENT_NOT_FOUND`].
    ForkParentNotFound,
    /// The fork parent exists but has no saved, restorable state to seed the
    /// fork with. Maps to [`FORK_PARENT_STATE_UNAVAILABLE`].
    ForkParentStateUnavailable,
    /// A session-state operation (status/pin) targets a session with no saved
    /// state. Maps to [`SESSION_STATE_NOT_FOUND`].
    SessionStateNotFound,
    /// A `sessionId`-accepting method resolved its id to no live session. Maps
    /// to [`SESSION_NOT_FOUND`].
    SessionNotFound,
}

impl SessionErrorKind {
    /// The contract's wire string for this kind — the exact value that lands in
    /// the ACP error's `data.error` field, identical from both agents.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ForkParentNotFound => FORK_PARENT_NOT_FOUND,
            Self::ForkParentStateUnavailable => FORK_PARENT_STATE_UNAVAILABLE,
            Self::SessionStateNotFound => SESSION_STATE_NOT_FOUND,
            Self::SessionNotFound => SESSION_NOT_FOUND,
        }
    }
}

impl std::fmt::Display for SessionErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parameters of [`SESSION_FORK_METHOD`]: fork a new session from `parent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionForkRequest {
    /// The session to fork from. The new session starts with this session's
    /// conversation history and, when available, its saved generation state.
    pub parent_session_id: String,
}

/// Result of [`SESSION_FORK_METHOD`]: the new session plus what was attached.
///
/// `state_attached`/`prefix_tokens` exist so a client can detect a degraded
/// fork (history cloned but no reusable state) instead of silently paying a
/// cold prefill — though an agent may equally reject such a fork outright with
/// [`FORK_PARENT_STATE_UNAVAILABLE`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionForkResponse {
    /// The id of the newly created (forked) session.
    pub session_id: String,
    /// Whether the parent's saved generation state was attached to the fork.
    pub state_attached: bool,
    /// Number of prompt tokens the attached state covers — the strict prefix
    /// the fork's first decode resumes after. `None` when the backend does not
    /// track token counts (or no state was attached).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix_tokens: Option<u64>,
}

/// Parameters of [`SESSION_STATE_STATUS_METHOD`]: which session to inspect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStateStatusRequest {
    /// The session whose saved-state status is queried.
    pub session_id: String,
}

/// Result of [`SESSION_STATE_STATUS_METHOD`].
///
/// `saved` means a state snapshot exists for the session. `prompt_tokens`
/// being present additionally means the snapshot carries the prompt-token
/// fingerprint required to seed a strict-prefix fork — a client should treat
/// `saved && prompt_tokens.is_some()` as "forkable".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStateStatusResponse {
    /// Whether a state snapshot is saved for the session.
    pub saved: bool,
    /// Number of prompt tokens the saved state covers, when the backend tracks
    /// it. `None` when no state is saved or the backend has no token counts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_tokens: Option<u64>,
    /// Size of the saved state in bytes, when the backend tracks it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes: Option<u64>,
    /// Whether the saved state is pinned against cache eviction.
    pub pinned: bool,
}

/// Parameters of [`SESSION_PIN_METHOD`]: pin or unpin a session's saved state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPinRequest {
    /// The session whose saved state should be pinned/unpinned.
    pub session_id: String,
    /// `true` to pin, `false` to unpin.
    pub pinned: bool,
}

/// Result of [`SESSION_PIN_METHOD`]: the *effective* pin state after the call.
///
/// Pinning is allowed to be a no-op: a backend without eviction (or without
/// pinning) reports `pinned: false` for a pin request rather than pretending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPinResponse {
    /// Whether the session's saved state is pinned after this call.
    pub pinned: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fork request/response serialize with the camelCase field names both
    /// ends of the extension contract expect on the wire.
    #[test]
    fn fork_types_use_camel_case_wire_shape() {
        let request = SessionForkRequest {
            parent_session_id: "parent-1".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            serde_json::json!({"parentSessionId": "parent-1"})
        );

        let response = SessionForkResponse {
            session_id: "child-1".to_string(),
            state_attached: true,
            prefix_tokens: Some(15000),
        };
        assert_eq!(
            serde_json::to_value(&response).unwrap(),
            serde_json::json!({
                "sessionId": "child-1",
                "stateAttached": true,
                "prefixTokens": 15000
            })
        );
    }

    /// `prefix_tokens` is optional on the wire: omitted when absent, and a
    /// response without it deserializes to `None` (backend-agnostic contract).
    #[test]
    fn fork_response_prefix_tokens_is_optional() {
        let response = SessionForkResponse {
            session_id: "child-1".to_string(),
            state_attached: false,
            prefix_tokens: None,
        };
        let value = serde_json::to_value(&response).unwrap();
        assert!(value.get("prefixTokens").is_none());

        let parsed: SessionForkResponse = serde_json::from_value(serde_json::json!({
            "sessionId": "child-1",
            "stateAttached": false
        }))
        .unwrap();
        assert_eq!(parsed, response);
    }

    /// The state-status types round-trip through their camelCase wire shape,
    /// with the optional counts omitted when the backend has none.
    #[test]
    fn state_status_types_round_trip() {
        let request: SessionStateStatusRequest =
            serde_json::from_value(serde_json::json!({"sessionId": "s-1"})).unwrap();
        assert_eq!(request.session_id, "s-1");

        let saved = SessionStateStatusResponse {
            saved: true,
            prompt_tokens: Some(123),
            bytes: Some(4096),
            pinned: true,
        };
        assert_eq!(
            serde_json::to_value(&saved).unwrap(),
            serde_json::json!({
                "saved": true,
                "promptTokens": 123,
                "bytes": 4096,
                "pinned": true
            })
        );

        let unsaved = SessionStateStatusResponse {
            saved: false,
            prompt_tokens: None,
            bytes: None,
            pinned: false,
        };
        let value = serde_json::to_value(&unsaved).unwrap();
        assert_eq!(value, serde_json::json!({"saved": false, "pinned": false}));
        let parsed: SessionStateStatusResponse = serde_json::from_value(value).unwrap();
        assert_eq!(parsed, unsaved);
    }

    /// Every `SessionErrorKind` variant maps to its contract wire constant, so
    /// the typed kind is interchangeable with the bare `&str` it replaces and a
    /// client branching on `data.error` sees the identical value.
    #[test]
    fn session_error_kind_maps_to_contract_constants() {
        assert_eq!(
            SessionErrorKind::ForkParentNotFound.as_str(),
            FORK_PARENT_NOT_FOUND
        );
        assert_eq!(
            SessionErrorKind::ForkParentStateUnavailable.as_str(),
            FORK_PARENT_STATE_UNAVAILABLE
        );
        assert_eq!(
            SessionErrorKind::SessionStateNotFound.as_str(),
            SESSION_STATE_NOT_FOUND
        );
        assert_eq!(
            SessionErrorKind::SessionNotFound.as_str(),
            SESSION_NOT_FOUND
        );
        // Display matches as_str so it can be formatted into log lines directly.
        assert_eq!(
            SessionErrorKind::ForkParentNotFound.to_string(),
            FORK_PARENT_NOT_FOUND
        );
    }

    /// The pin types round-trip through their camelCase wire shape.
    #[test]
    fn pin_types_round_trip() {
        let request: SessionPinRequest =
            serde_json::from_value(serde_json::json!({"sessionId": "s-1", "pinned": true}))
                .unwrap();
        assert_eq!(request.session_id, "s-1");
        assert!(request.pinned);

        let response = SessionPinResponse { pinned: true };
        assert_eq!(
            serde_json::to_value(&response).unwrap(),
            serde_json::json!({"pinned": true})
        );
    }
}
