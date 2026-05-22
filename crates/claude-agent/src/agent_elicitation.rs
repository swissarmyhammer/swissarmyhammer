//! Agent-side handler that relays Claude CLI elicitations to the ACP client.
//!
//! The streaming loop in [`crate::claude`] recognizes a CLI elicitation
//! control request but does not hold the ACP client connection. This module
//! provides [`ElicitationBridgeHandler`], the implementation of
//! [`crate::elicitation_bridge::ElicitationHandler`] that
//! [`crate::agent::ClaudeAgent`] installs on its [`crate::claude::ClaudeClient`].
//! It performs the `elicitation/create` round-trip over the agent's stored
//! `ConnectionTo<Client>` handle — exactly mirroring the permission round-trip
//! in [`crate::agent::ClaudeAgent::request_client_permission`].
//!
//! # Shared state
//!
//! Both the client connection and the client capabilities are populated *after*
//! the handler is built (the connection is set when the transport connects; the
//! capabilities arrive in the `initialize` request). The handler therefore
//! reads them through `Arc<RwLock<...>>` cells shared with the agent, rather
//! than capturing snapshots at construction time.

use std::sync::Arc;

use agent_client_protocol::schema::{
    ClientCapabilities, CreateElicitationResponse, ElicitationAction, SessionId,
};
use agent_client_protocol::{Client, ConnectionTo, UntypedMessage};
use tokio::sync::RwLock;

use crate::elicitation_bridge::{CliElicitationRequest, ElicitationHandler, ElicitationOutcome};

/// The ACP method name for client-bound elicitation requests.
///
/// Matches `agent_client_protocol_schema::ELICITATION_CREATE_METHOD_NAME` and
/// the webview client. The elicitation types are reachable in this workspace
/// but the main `agent-client-protocol` crate at 0.11 does not implement
/// `JsonRpcRequest` for `CreateElicitationRequest` (that wiring is gated behind
/// the main crate's own unstable feature, which 0.11 does not forward). The
/// request is therefore dispatched as an [`UntypedMessage`] carrying the
/// serialized ACP request as params, exactly as the typed path would on the
/// wire.
const ELICITATION_CREATE_METHOD: &str = "elicitation/create";

/// Shared cell holding the ACP client connection once the transport is wired.
///
/// Mirrors the agent's own client storage so the handler always sees the live
/// connection, including the case where the connection is set after the handler
/// has been installed on the Claude client.
pub type SharedClient = Arc<RwLock<Option<ConnectionTo<Client>>>>;

/// Shared cell holding the client capabilities reported during `initialize`.
pub type SharedClientCapabilities = Arc<RwLock<Option<ClientCapabilities>>>;

/// Relays CLI elicitations to the ACP client via `elicitation/create`.
///
/// Constructed by the agent with shared handles to the client connection and
/// the client capabilities, and installed on the Claude client so the streaming
/// loop can complete the round-trip.
pub struct ElicitationBridgeHandler {
    client: SharedClient,
    client_capabilities: SharedClientCapabilities,
}

impl ElicitationBridgeHandler {
    /// Create a new handler over the agent's shared client and capability cells.
    ///
    /// # Arguments
    ///
    /// * `client` - Shared cell with the ACP client connection (set on connect).
    /// * `client_capabilities` - Shared cell with the client's capabilities
    ///   (set when the `initialize` request arrives).
    pub fn new(client: SharedClient, client_capabilities: SharedClientCapabilities) -> Self {
        Self {
            client,
            client_capabilities,
        }
    }

    /// Whether the client advertised any form of elicitation support.
    ///
    /// The bridge only ever issues form-mode elicitations, so the presence of
    /// the `elicitation` capability object is sufficient. A client that did not
    /// advertise the capability is not asked — it is declined instead, so the
    /// CLI is unblocked promptly rather than waiting on a request the client
    /// will not service.
    async fn client_supports_elicitation(&self) -> bool {
        let guard = self.client_capabilities.read().await;
        guard
            .as_ref()
            .map(|caps| caps.elicitation.is_some())
            .unwrap_or(false)
    }
}

#[async_trait::async_trait]
impl ElicitationHandler for ElicitationBridgeHandler {
    async fn handle_elicitation(
        &self,
        request: &CliElicitationRequest,
        session_id: SessionId,
    ) -> ElicitationOutcome {
        // This bridge only renders form-mode elicitations against the ACP
        // client. A `url`-mode elicitation cannot be presented as a form, so it
        // is declined explicitly rather than silently relayed as an empty form
        // the user could never meaningfully answer — the CLI is unblocked with a
        // clear non-answer.
        if !request.is_form_mode() {
            tracing::warn!(
                "Unsupported elicitation mode {:?}; declining request_id={}",
                request.mode,
                request.request_id
            );
            return declined();
        }

        if !self.client_supports_elicitation().await {
            tracing::warn!(
                "Client did not advertise the elicitation capability; declining request_id={}",
                request.request_id
            );
            return declined();
        }

        let client_guard = self.client.read().await;
        let Some(client) = client_guard.as_ref() else {
            tracing::warn!(
                "No client connection available; declining elicitation request_id={}",
                request.request_id
            );
            return declined();
        };

        let acp_request = request.to_acp_request(session_id);
        let untyped = match UntypedMessage::new(ELICITATION_CREATE_METHOD, &acp_request) {
            Ok(message) => message,
            Err(e) => {
                tracing::error!("Failed to encode elicitation/create request: {}", e);
                return ElicitationOutcome::Error(format!(
                    "failed to encode elicitation/create request: {e}"
                ));
            }
        };

        // ACP 0.11: dispatch to the counterpart Client role over the stored
        // `ConnectionTo<Client>` handle. `block_task` is safe here only because
        // the prompt turn that drives this code runs *off* the connection's
        // dispatch loop (the prompt request is dispatched via `cx.spawn` in
        // `swissarmyhammer-agent::dispatch_claude_request`). If the prompt turn
        // were awaited inline on the dispatch loop, that loop could never route
        // this request's response back here and the await would hang forever —
        // the original production bug. `UntypedMessage::Response` is a raw JSON
        // value, deserialized into the concrete ACP response type below.
        //
        // Log the await boundary so the unified log shows when we hand the
        // elicitation to the client and when (or whether) the answer comes back.
        tracing::info!(
            "Relaying elicitation to client; awaiting response request_id={}",
            request.request_id
        );
        match client.send_request(untyped).block_task().await {
            Ok(value) => match serde_json::from_value::<CreateElicitationResponse>(value) {
                Ok(response) => {
                    tracing::info!(
                        "Elicitation answered action={:?} request_id={}",
                        response.action,
                        request.request_id
                    );
                    ElicitationOutcome::Responded(response)
                }
                Err(e) => {
                    tracing::error!("Failed to decode elicitation/create response: {}", e);
                    ElicitationOutcome::Error(format!(
                        "failed to decode elicitation/create response: {e}"
                    ))
                }
            },
            // A transport/infrastructure failure is reported as an error rather
            // than mapped to a cancel: a cancel would reach the CLI as a clean
            // user cancellation (a `success` envelope), hiding the real failure.
            // Only a genuine ACP Cancel *action* from the client maps to cancel.
            Err(e) => {
                tracing::error!("Failed to relay elicitation to client: {}", e);
                ElicitationOutcome::Error(format!("failed to relay elicitation to client: {e}"))
            }
        }
    }
}

/// Build a `Responded(decline)` outcome.
///
/// Used for the local non-error declines: an unsupported mode, a missing client
/// connection, or a client that did not advertise the elicitation capability.
/// These are genuine answers (the elicitation is declined), not failures, so
/// they ride the `success` envelope.
fn declined() -> ElicitationOutcome {
    ElicitationOutcome::Responded(CreateElicitationResponse::new(ElicitationAction::Decline))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::schema::{ElicitationCapabilities, ElicitationFormCapabilities};
    use serde_json::json;

    fn sample_request() -> CliElicitationRequest {
        CliElicitationRequest::parse(
            &json!({
                "type": "control_request",
                "request_id": "req_x",
                "request": { "subtype": "elicitation", "message": "hi" }
            })
            .to_string(),
        )
        .expect("sample elicitation must parse")
    }

    /// Assert an outcome is a `Responded(decline)`.
    fn assert_declined(outcome: &ElicitationOutcome) {
        match outcome {
            ElicitationOutcome::Responded(response) => {
                assert!(
                    matches!(response.action, ElicitationAction::Decline),
                    "expected a decline response, got {:?}",
                    response.action
                );
            }
            other => panic!("expected Responded(decline), got {other:?}"),
        }
    }

    /// With no client capabilities recorded, the handler declines without
    /// touching the (absent) client connection.
    #[tokio::test]
    async fn declines_when_capability_absent() {
        let handler =
            ElicitationBridgeHandler::new(Arc::new(RwLock::new(None)), Arc::new(RwLock::new(None)));

        let outcome = handler
            .handle_elicitation(&sample_request(), SessionId::new("s"))
            .await;

        assert_declined(&outcome);
    }

    /// Capability advertised but no connection wired still declines (rather
    /// than panicking or hanging).
    #[tokio::test]
    async fn declines_when_capability_present_but_no_client() {
        let caps = ClientCapabilities::new()
            .elicitation(ElicitationCapabilities::new().form(ElicitationFormCapabilities::new()));
        let handler = ElicitationBridgeHandler::new(
            Arc::new(RwLock::new(None)),
            Arc::new(RwLock::new(Some(caps))),
        );

        let outcome = handler
            .handle_elicitation(&sample_request(), SessionId::new("s"))
            .await;

        assert_declined(&outcome);
    }

    /// A `url`-mode elicitation cannot be rendered as a form; the handler
    /// declines it explicitly rather than relaying an empty form, and it does so
    /// before consulting capabilities or the client connection.
    #[tokio::test]
    async fn declines_url_mode_elicitation() {
        let caps = ClientCapabilities::new()
            .elicitation(ElicitationCapabilities::new().form(ElicitationFormCapabilities::new()));
        let handler = ElicitationBridgeHandler::new(
            Arc::new(RwLock::new(None)),
            Arc::new(RwLock::new(Some(caps))),
        );

        let url_request = CliElicitationRequest::parse(
            &json!({
                "type": "control_request",
                "request_id": "req_url",
                "request": {
                    "subtype": "elicitation",
                    "message": "open this",
                    "mode": "url",
                    "url": "https://example.com"
                }
            })
            .to_string(),
        )
        .expect("url-mode elicitation must parse");

        let outcome = handler
            .handle_elicitation(&url_request, SessionId::new("s"))
            .await;

        assert_declined(&outcome);
    }
}
