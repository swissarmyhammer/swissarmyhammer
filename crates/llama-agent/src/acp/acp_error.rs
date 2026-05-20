//! Shared ACP error-construction helpers.
//!
//! This module is the single error-construction convention for the ACP
//! handler layer. Every handler builds JSON-RPC errors through one of these
//! named helpers rather than reaching for `agent_client_protocol::Error::new`
//! with a raw integer code.
//!
//! Why a helper layer instead of `Error::new(-32602, ..)`:
//!
//! * **No raw integer codes.** A code like `-32602` carries no meaning at the
//!   call site. `invalid_params(..)` names the JSON-RPC class.
//! * **Custom diagnostic messages survive.** The ACP named constructors
//!   (`Error::invalid_params()`, etc.) hard-code a terse message. These helpers
//!   take the JSON-RPC code from the named [`agent_client_protocol::ErrorCode`]
//!   constant while letting the caller supply a descriptive message and
//!   structured `data`.
//! * **Consistency across both agents.** `claude-agent` carries an identical
//!   `acp_error` module, so the same failure class produces the same error
//!   code and shape from either agent.
//!
//! Errors that originate from a typed error enum
//! ([`TerminalError`](super::error::TerminalError),
//! [`crate::types::AgentError`], etc.) are mapped through
//! [`ToJsonRpcError`](super::translation::ToJsonRpcError) and
//! `AcpServer::convert_error` instead — that path already names the code via
//! the enum's `to_json_rpc_code`. These helpers are for handler-local errors
//! that have no backing error type.

use agent_client_protocol::{Error, ErrorCode};

/// Build an error with a named JSON-RPC code and a custom message.
///
/// The code comes from a strongly-typed [`ErrorCode`] constant; the message is
/// caller-supplied so handler-specific guidance is not lost.
fn with_message(code: ErrorCode, message: impl Into<String>) -> Error {
    let mut error: Error = code.into();
    error.message = message.into();
    error
}

/// `-32600 Invalid Request` with a custom message.
///
/// Use when the request is structurally well-formed JSON-RPC but not a valid
/// request for this method (e.g. a capability prerequisite is unmet).
#[must_use]
pub fn invalid_request(message: impl Into<String>) -> Error {
    with_message(ErrorCode::InvalidRequest, message)
}

/// `-32601 Method Not Found` with a custom message.
///
/// Use for unknown extension methods and for methods gated behind a capability
/// the agent does not advertise.
#[must_use]
pub fn method_not_found(message: impl Into<String>) -> Error {
    with_message(ErrorCode::MethodNotFound, message)
}

/// `-32602 Invalid Params` with a custom message.
///
/// Use when a parameter is missing, malformed, or fails validation — including
/// capability-gating failures where the client did not declare a capability it
/// is now exercising.
#[must_use]
pub fn invalid_params(message: impl Into<String>) -> Error {
    with_message(ErrorCode::InvalidParams, message)
}

/// `-32603 Internal Error` with a custom message.
///
/// Use for unexpected server-side failures (serialization errors, lock
/// poisoning, and similar conditions the client cannot correct).
#[must_use]
pub fn internal_error(message: impl Into<String>) -> Error {
    with_message(ErrorCode::InternalError, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_helpers_carry_the_expected_code_and_message() {
        let err = invalid_params("bad path");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert_eq!(err.message, "bad path");
        assert!(err.data.is_none());

        assert_eq!(
            invalid_request("not allowed").code,
            ErrorCode::InvalidRequest
        );
        assert_eq!(
            method_not_found("no such method").code,
            ErrorCode::MethodNotFound
        );
        assert_eq!(internal_error("boom").code, ErrorCode::InternalError);
    }

    #[test]
    fn data_is_chainable_via_the_acp_builder() {
        let err = invalid_params("bad path").data(serde_json::json!({ "field": "path" }));
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert_eq!(err.data.unwrap()["field"], "path");
    }
}
