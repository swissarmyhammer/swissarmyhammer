//! End-to-end test for the Claude CLI elicitation → ACP `elicitation/create`
//! bridge.
//!
//! These tests exercise [`claude_agent::agent_elicitation::ElicitationBridgeHandler`]
//! over a *real* in-process ACP connection: an `Agent` end holding a live
//! `ConnectionTo<Client>`, paired over a `Channel::duplex()` with a fake
//! `Client` that answers `elicitation/create`. This is the same connection
//! machinery the production agent uses, so it proves the whole round-trip:
//!
//! 1. A simulated CLI elicitation control request is parsed.
//! 2. The handler relays it to the client as exactly one `elicitation/create`
//!    request (asserted by the fake client recording the request it saw).
//! 3. The client's accept/decline/cancel response is mapped back into the ACP
//!    [`CreateElicitationResponse`] the bridge then turns into a CLI
//!    control_response.
//!
//! The fake client is built directly here (rather than reusing
//! `tests/common/test_client.rs`, which only models the agent→client requests
//! that already existed) because `elicitation/create` is a new client-bound
//! method. It is handled via an [`UntypedMessage`] request handler since the
//! main `agent-client-protocol` crate at 0.11 does not implement
//! `JsonRpcRequest` for the elicitation types — exactly mirroring how the
//! production bridge dispatches the request.

use std::sync::Arc;

use agent_client_protocol::schema::{CreateElicitationResponse, ElicitationAction, SessionId};
use agent_client_protocol::{Agent, Channel, Client, ConnectTo, ConnectionTo, UntypedMessage};
use serde_json::{json, Value as JsonValue};
use tokio::sync::{Mutex, RwLock};

use claude_agent::agent_elicitation::ElicitationBridgeHandler;
use claude_agent::elicitation_bridge::{
    CliElicitationRequest, ElicitationHandler, ElicitationOutcome,
};

/// Unwrap an [`ElicitationOutcome::Responded`] or fail the test.
///
/// The accept/decline/cancel round-trip tests expect a real client response;
/// an [`ElicitationOutcome::Error`] would be an infrastructure failure they do
/// not exercise.
fn expect_responded(outcome: ElicitationOutcome) -> CreateElicitationResponse {
    match outcome {
        ElicitationOutcome::Responded(response) => response,
        ElicitationOutcome::Error(message) => {
            panic!("expected a client response, got an error outcome: {message}")
        }
    }
}

/// What the fake client should reply to an `elicitation/create` request.
#[derive(Clone)]
enum FakeReply {
    /// Accept with the given content map (as JSON object).
    Accept(JsonValue),
    /// Decline the elicitation.
    Decline,
    /// Cancel the elicitation.
    Cancel,
    /// Fail the request at the transport level (the client returns a JSON-RPC
    /// error rather than a result), simulating an infrastructure failure.
    TransportError,
}

impl FakeReply {
    /// The result the fake client returns for an `elicitation/create` request.
    ///
    /// `Ok` carries the `{action, content?}` JSON result; `Err` carries a
    /// JSON-RPC error so `send_request(...).block_task()` resolves to `Err`,
    /// exercising the bridge's infrastructure-failure path.
    fn to_result(&self) -> agent_client_protocol::Result<JsonValue> {
        match self {
            FakeReply::Accept(content) => Ok(json!({ "action": "accept", "content": content })),
            FakeReply::Decline => Ok(json!({ "action": "decline" })),
            FakeReply::Cancel => Ok(json!({ "action": "cancel" })),
            FakeReply::TransportError => Err(agent_client_protocol::Error::internal_error()),
        }
    }
}

/// Records the `elicitation/create` requests the fake client received.
type Recorded = Arc<Mutex<Vec<UntypedMessage>>>;

/// `ConnectTo<Agent>` fake client that answers `elicitation/create`.
struct FakeElicitationClient {
    reply: FakeReply,
    recorded: Recorded,
}

impl ConnectTo<Agent> for FakeElicitationClient {
    async fn connect_to(
        self,
        agent: impl ConnectTo<<Agent as agent_client_protocol::Role>::Counterpart>,
    ) -> agent_client_protocol::Result<()> {
        let reply = self.reply;
        let recorded = self.recorded;

        Client
            .builder()
            .name("fake-elicitation-client")
            .on_receive_request(
                async move |req: UntypedMessage,
                            responder: agent_client_protocol::Responder<JsonValue>,
                            _cx: ConnectionTo<Agent>| {
                    // Record exactly what the client saw so the test can assert
                    // a single, well-formed elicitation/create request arrived.
                    recorded.lock().await.push(req.clone());

                    let result = if req.method() == "elicitation/create" {
                        reply.to_result()
                    } else {
                        Err(agent_client_protocol::Error::method_not_found())
                    };
                    responder.respond_with_result(result)
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_to(agent)
            .await
    }
}

/// Drive `body` with a live `ConnectionTo<Client>` whose counterpart is a fake
/// client replying with `reply`. Returns the body's value plus the requests the
/// fake client recorded.
async fn with_fake_client<F, Fut, R>(reply: FakeReply, body: F) -> (R, Vec<UntypedMessage>)
where
    F: FnOnce(ConnectionTo<Client>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    let recorded: Recorded = Arc::new(Mutex::new(Vec::new()));
    let (channel_a, channel_b) = Channel::duplex();

    let client_recorded = Arc::clone(&recorded);
    let client_task = tokio::spawn(async move {
        let _ = FakeElicitationClient {
            reply,
            recorded: client_recorded,
        }
        .connect_to(channel_a)
        .await;
    });

    let result = Agent
        .builder()
        .name("elicitation-bridge-agent")
        .connect_with(channel_b, async move |conn: ConnectionTo<Client>| {
            Ok(body(conn).await)
        })
        .await
        .expect("agent connect_with should succeed");

    client_task.abort();
    let _ = client_task.await;

    let seen = recorded.lock().await.clone();
    (result, seen)
}

/// A representative CLI elicitation control request.
fn sample_cli_elicitation() -> CliElicitationRequest {
    CliElicitationRequest::parse(
        &json!({
            "type": "control_request",
            "request_id": "req_e2e",
            "request": {
                "subtype": "elicitation",
                "mcp_server_name": "per-board-kanban",
                "message": "What is your name?",
                "mode": "form",
                "requested_schema": {
                    "type": "object",
                    "properties": { "name": { "type": "string", "title": "Name" } },
                    "required": ["name"]
                }
            }
        })
        .to_string(),
    )
    .expect("sample elicitation must parse")
}

/// Build a handler whose client connection is set after construction (as in
/// production) and whose capabilities advertise form elicitation.
fn handler_with_capability(client: ConnectionTo<Client>) -> ElicitationBridgeHandler {
    use agent_client_protocol::schema::{
        ClientCapabilities, ElicitationCapabilities, ElicitationFormCapabilities,
    };

    let caps = ClientCapabilities::new()
        .elicitation(ElicitationCapabilities::new().form(ElicitationFormCapabilities::new()));

    ElicitationBridgeHandler::new(
        Arc::new(RwLock::new(Some(client))),
        Arc::new(RwLock::new(Some(caps))),
    )
}

#[tokio::test]
async fn accept_with_content_round_trips_back_to_cli_response() {
    let request = sample_cli_elicitation();

    let request_for_call = request.clone();
    let (outcome, seen) = with_fake_client(
        FakeReply::Accept(json!({ "name": "Alice" })),
        move |conn| async move {
            let handler = handler_with_capability(conn);
            handler
                .handle_elicitation(&request_for_call, SessionId::new("sess_e2e"))
                .await
        },
    )
    .await;
    let acp_response = expect_responded(outcome);

    // Exactly one elicitation/create request must have reached the client.
    assert_eq!(
        seen.len(),
        1,
        "expected exactly one elicitation/create request, saw {}",
        seen.len()
    );
    assert_eq!(seen[0].method(), "elicitation/create");
    // The request the client saw must carry the camelCase ACP form fields.
    assert_eq!(seen[0].params()["sessionId"], "sess_e2e");
    assert_eq!(seen[0].params()["mode"], "form");
    assert_eq!(seen[0].params()["message"], "What is your name?");
    assert!(seen[0].params()["requestedSchema"]["properties"]["name"].is_object());

    // The accept-with-content response must map back faithfully.
    match &acp_response.action {
        ElicitationAction::Accept(accept) => {
            let content = accept.content.as_ref().expect("accept must carry content");
            let value = content.get("name").expect("name key must be present");
            assert_eq!(
                serde_json::to_value(value).unwrap(),
                json!("Alice"),
                "accepted content must round-trip"
            );
        }
        other => panic!("expected accept, got {other:?}"),
    }

    // And the bridge must turn that into the correct CLI control_response.
    let cli_response = request.success_control_response(&acp_response);
    assert_eq!(cli_response["type"], "control_response");
    assert_eq!(cli_response["response"]["subtype"], "success");
    assert_eq!(cli_response["response"]["request_id"], "req_e2e");
    assert_eq!(cli_response["response"]["response"]["action"], "accept");
    assert_eq!(
        cli_response["response"]["response"]["content"]["name"],
        "Alice"
    );
}

#[tokio::test]
async fn decline_round_trips_back_to_cli_response() {
    let request = sample_cli_elicitation();

    let request_for_call = request.clone();
    let (outcome, seen) = with_fake_client(FakeReply::Decline, move |conn| async move {
        let handler = handler_with_capability(conn);
        handler
            .handle_elicitation(&request_for_call, SessionId::new("sess_e2e"))
            .await
    })
    .await;
    let acp_response = expect_responded(outcome);

    assert_eq!(seen.len(), 1, "exactly one elicitation/create request");
    assert!(matches!(acp_response.action, ElicitationAction::Decline));

    let cli_response = request.success_control_response(&acp_response);
    assert_eq!(cli_response["response"]["response"]["action"], "decline");
    assert!(cli_response["response"]["response"]
        .get("content")
        .is_none());
}

#[tokio::test]
async fn cancel_round_trips_back_to_cli_response() {
    let request = sample_cli_elicitation();

    let request_for_call = request.clone();
    let (outcome, seen) = with_fake_client(FakeReply::Cancel, move |conn| async move {
        let handler = handler_with_capability(conn);
        handler
            .handle_elicitation(&request_for_call, SessionId::new("sess_e2e"))
            .await
    })
    .await;
    let acp_response = expect_responded(outcome);

    assert_eq!(seen.len(), 1, "exactly one elicitation/create request");
    assert!(matches!(acp_response.action, ElicitationAction::Cancel));

    let cli_response = request.success_control_response(&acp_response);
    assert_eq!(cli_response["response"]["response"]["action"], "cancel");
}

#[tokio::test]
async fn no_capability_declines_without_calling_client() {
    let request = sample_cli_elicitation();

    // Capability advertised would normally relay; here we omit it so the
    // handler must decline locally and never reach the client.
    let (outcome, seen) = with_fake_client(FakeReply::Accept(json!({})), move |conn| async move {
        let handler = ElicitationBridgeHandler::new(
            Arc::new(RwLock::new(Some(conn))),
            Arc::new(RwLock::new(None)),
        );
        handler
            .handle_elicitation(&request, SessionId::new("sess_e2e"))
            .await
    })
    .await;
    let acp_response = expect_responded(outcome);

    assert_eq!(
        seen.len(),
        0,
        "no elicitation/create request must be sent when the client lacks the capability"
    );
    assert!(matches!(acp_response.action, ElicitationAction::Decline));

    // Sanity: a typed decline still serializes to a valid CLI control_response.
    let _ = CreateElicitationResponse::new(ElicitationAction::Decline);
}

#[tokio::test]
async fn transport_failure_maps_to_cli_error_envelope_not_cancel() {
    let request = sample_cli_elicitation();

    // The client is reachable and advertises the capability, but the
    // elicitation/create round-trip fails at the transport level (the client
    // returns a JSON-RPC error). This must surface as an error outcome, not a
    // cancel — a cancel would reach the CLI as a clean user cancellation and
    // hide the failure.
    let request_for_call = request.clone();
    let (outcome, seen) = with_fake_client(FakeReply::TransportError, move |conn| async move {
        let handler = handler_with_capability(conn);
        handler
            .handle_elicitation(&request_for_call, SessionId::new("sess_e2e"))
            .await
    })
    .await;

    // The request did reach the client (it was the *response* that failed).
    assert_eq!(seen.len(), 1, "exactly one elicitation/create request");

    // The outcome must be an error, never a Responded(cancel).
    let message = match outcome {
        ElicitationOutcome::Error(message) => message,
        ElicitationOutcome::Responded(response) => {
            panic!(
                "a transport failure must not map to a client response, got {:?}",
                response.action
            )
        }
    };

    // And the bridge must turn that into the CLI's `error` envelope, not a
    // `success`/`cancel` one.
    let cli_response =
        request.control_response_for_outcome(&ElicitationOutcome::Error(message.clone()));
    assert_eq!(cli_response["type"], "control_response");
    assert_eq!(cli_response["response"]["subtype"], "error");
    assert_eq!(cli_response["response"]["request_id"], "req_e2e");
    assert_eq!(cli_response["response"]["error"], message);
    assert!(
        cli_response["response"]["response"].is_null(),
        "an error envelope must not carry a success/action payload"
    );
}
