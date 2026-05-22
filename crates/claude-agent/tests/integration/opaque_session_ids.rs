//! Tests for the opaque-session-id contract on the live request handlers.
//!
//! A session id is an **opaque string**: it is valid if and only if a session
//! with that exact id exists, never because it matches a ULID format. Every
//! method that accepts a `sessionId` — `session/prompt`, `session/cancel`,
//! `session/set_mode`, and the `terminal/*` extension handlers — resolves the
//! id the same way, and an unknown id fails with one uniform `invalid_params`
//! error.
//!
//! These tests assert that uniformity directly:
//!
//! 1. A non-ULID id is **not** rejected on format — it fails as a not-found
//!    lookup, the same outcome as a well-formed ULID with no live session.
//! 2. `prompt`, `cancel`, and `set_session_mode` all return the identical
//!    `invalid_params` code for an unknown id. In particular `set_session_mode`
//!    no longer returns the old `invalid_request` code.
//! 3. The `terminal/*` extension handlers return the same `invalid_params`
//!    code — not the old `-32600` `Invalid Request` — for an unknown id.
//!
//! The handlers reject the unknown id before any backend work, so every
//! assertion here is deterministic and never spawns the claude CLI.

use std::sync::Arc;

use agent_client_protocol::schema::{
    CancelNotification, ClientCapabilities, ContentBlock, ExtRequest, InitializeRequest,
    PromptRequest, SessionId, SessionModeId, SetSessionModeRequest, TextContent,
};
use agent_client_protocol::ErrorCode;
use claude_agent::{config::AgentConfig, ClaudeAgent};
use serde_json::value::RawValue;

/// A well-formed ULID for which no session is ever created — the "ULID, but no
/// live session" miss.
const UNKNOWN_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

/// A perfectly valid opaque session id that is deliberately *not* a ULID — the
/// id a non-claude client might mint. It must miss exactly like `UNKNOWN_ULID`.
const NON_ULID_ID: &str = "my-custom-session";

/// Build a minimal one-text-block prompt request for `session_id`.
fn prompt_request(session_id: &str) -> PromptRequest {
    PromptRequest::new(
        SessionId::new(session_id),
        vec![ContentBlock::Text(TextContent::new("hello".to_string()))],
    )
}

/// `session/prompt` rejects an unknown id — whether it is a non-ULID string or
/// a well-formed ULID with no live session — with the same `invalid_params`
/// error. The non-ULID id is never rejected up front on format.
#[tokio::test]
async fn prompt_rejects_unknown_session_id_uniformly() {
    let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

    let non_ulid_err = agent
        .prompt(prompt_request(NON_ULID_ID))
        .await
        .expect_err("prompt for a non-ULID unknown id must fail");
    assert_eq!(
        non_ulid_err.code,
        ErrorCode::InvalidParams,
        "a non-ULID id must miss as not-found, never be format-rejected"
    );

    let ulid_err = agent
        .prompt(prompt_request(UNKNOWN_ULID))
        .await
        .expect_err("prompt for an unknown ULID must fail");
    assert_eq!(
        ulid_err.code,
        ErrorCode::InvalidParams,
        "an unknown ULID must miss as not-found"
    );

    assert_eq!(
        non_ulid_err.code, ulid_err.code,
        "both kinds of unknown id must fail with the same error code"
    );
}

/// `session/cancel` resolves the id the same way as `prompt`: an unknown id —
/// non-ULID or ULID — fails with `invalid_params` before any cancellation work.
#[tokio::test]
async fn cancel_rejects_unknown_session_id_uniformly() {
    let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

    let non_ulid_err = agent
        .cancel(CancelNotification::new(SessionId::new(NON_ULID_ID)))
        .await
        .expect_err("cancel for a non-ULID unknown id must fail");
    assert_eq!(non_ulid_err.code, ErrorCode::InvalidParams);

    let ulid_err = agent
        .cancel(CancelNotification::new(SessionId::new(UNKNOWN_ULID)))
        .await
        .expect_err("cancel for an unknown ULID must fail");
    assert_eq!(ulid_err.code, ErrorCode::InvalidParams);

    assert_eq!(
        non_ulid_err.code, ulid_err.code,
        "cancel must fail the same way for both kinds of unknown id"
    );
}

/// `session/set_mode` resolves the id the same way: an unknown id fails with
/// `invalid_params`. This is the key consistency fix — the handler previously
/// returned `invalid_request` for a bad session id, a different code for the
/// same failure.
#[tokio::test]
async fn set_session_mode_rejects_unknown_session_id_with_invalid_params() {
    let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

    let mode = SessionModeId::new("code");

    let non_ulid_err = agent
        .set_session_mode(SetSessionModeRequest::new(
            SessionId::new(NON_ULID_ID),
            mode.clone(),
        ))
        .await
        .expect_err("set_session_mode for a non-ULID unknown id must fail");
    assert_eq!(
        non_ulid_err.code,
        ErrorCode::InvalidParams,
        "set_session_mode must use invalid_params, not the old invalid_request"
    );

    let ulid_err = agent
        .set_session_mode(SetSessionModeRequest::new(
            SessionId::new(UNKNOWN_ULID),
            mode,
        ))
        .await
        .expect_err("set_session_mode for an unknown ULID must fail");
    assert_eq!(
        ulid_err.code,
        ErrorCode::InvalidParams,
        "set_session_mode must use invalid_params, not the old invalid_request"
    );
}

/// Every `sessionId`-accepting handler returns the *identical* error code for
/// the same unknown id — the one-error, one-code guarantee across `prompt`,
/// `cancel`, and `set_session_mode`.
#[tokio::test]
async fn all_handlers_share_one_not_found_error_code() {
    let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

    let prompt_code = agent
        .prompt(prompt_request(NON_ULID_ID))
        .await
        .expect_err("prompt must reject the unknown id")
        .code;
    let cancel_code = agent
        .cancel(CancelNotification::new(SessionId::new(NON_ULID_ID)))
        .await
        .expect_err("cancel must reject the unknown id")
        .code;
    let set_mode_code = agent
        .set_session_mode(SetSessionModeRequest::new(
            SessionId::new(NON_ULID_ID),
            SessionModeId::new("code"),
        ))
        .await
        .expect_err("set_session_mode must reject the unknown id")
        .code;

    assert_eq!(prompt_code, ErrorCode::InvalidParams);
    assert_eq!(
        prompt_code, cancel_code,
        "prompt and cancel must return the same not-found code"
    );
    assert_eq!(
        prompt_code, set_mode_code,
        "prompt and set_session_mode must return the same not-found code"
    );
}

/// Build a `terminal/release` extension request body for `session_id`.
///
/// The terminal id is irrelevant: the session id is resolved before any
/// terminal lookup, so an unknown session id fails first.
fn terminal_release_request(session_id: &str) -> ExtRequest {
    let params = serde_json::json!({
        "sessionId": session_id,
        "terminalId": "term_unused",
    });
    let raw = RawValue::from_string(params.to_string()).expect("params serialize");
    ExtRequest::new("terminal/release", Arc::from(raw))
}

/// The `terminal/*` extension handlers resolve a `sessionId` the same way as
/// every other handler: an unknown id — non-ULID or ULID — fails with
/// `invalid_params` (-32602), never the old `-32600` `Invalid Request` code
/// that `AgentError::Protocol` used to produce for terminal session lookups.
#[tokio::test]
async fn terminal_ext_handler_rejects_unknown_session_id_with_invalid_params() {
    let (agent, _rx) = ClaudeAgent::new(AgentConfig::default()).await.unwrap();

    // The terminal extension handlers are gated behind the terminal
    // capability; declare it so the call reaches the session resolver.
    agent
        .initialize(
            InitializeRequest::new(1.into())
                .client_capabilities(ClientCapabilities::new().terminal(true)),
        )
        .await
        .expect("initialize with terminal capability");

    let non_ulid_err = agent
        .ext_method(terminal_release_request(NON_ULID_ID))
        .await
        .expect_err("terminal/release for a non-ULID unknown id must fail");
    assert_eq!(
        non_ulid_err.code,
        ErrorCode::InvalidParams,
        "terminal handler must use invalid_params, not the old invalid_request"
    );

    let ulid_err = agent
        .ext_method(terminal_release_request(UNKNOWN_ULID))
        .await
        .expect_err("terminal/release for an unknown ULID must fail");
    assert_eq!(
        ulid_err.code,
        ErrorCode::InvalidParams,
        "terminal handler must use invalid_params for an unknown ULID too"
    );

    // The terminal handler must agree with the primary handlers on one code.
    let prompt_code = agent
        .prompt(prompt_request(NON_ULID_ID))
        .await
        .expect_err("prompt must reject the unknown id")
        .code;
    assert_eq!(
        non_ulid_err.code, prompt_code,
        "terminal and prompt must return the same not-found code"
    );
}
