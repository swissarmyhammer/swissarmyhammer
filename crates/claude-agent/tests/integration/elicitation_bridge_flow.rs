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

/// Reproduce the *production* elicitation context faithfully, including the
/// blocking `prompt` request handler that owns the dispatch loop.
///
/// [`with_fake_client`] runs the elicitation from the agent's `connect_with`
/// main_fn while the dispatch loop is otherwise idle, so its `block_task`
/// always resolves. Production is different in a decisive way:
///
/// * The agent registers an `on_receive_request` handler (`dispatch_claude_request`
///   → `agent.prompt(...).await`). Per the ACP SDK, `on_*` callbacks run
///   **inside the single dispatch loop**, and the loop is blocked until the
///   callback completes (see `agent_client_protocol::concepts::ordering`). The
///   same loop is what routes *incoming responses* back to `block_task`
///   awaiters.
/// * During that prompt turn, `query_stream` spawns `run_stream_loop` as a raw
///   `tokio::task::spawn` task. When it sees a CLI elicitation it issues
///   `client.send_request(elicitation/create).block_task().await` on a clone of
///   the stored `ConnectionTo<Client>`.
///
/// Because the dispatch loop is blocked inside the prompt handler, it can never
/// reach the branch that routes the `elicitation/create` response back to the
/// spawned task → `block_task` hangs forever. That is the production bug.
///
/// This helper models that topology exactly:
///
/// 1. The test side acts as the ACP **client**: it answers `elicitation/create`
///    (recording what it saw) and sends one "prompt" request to the agent.
/// 2. The agent side registers an `on_receive_request` handler for that prompt
///    request. Inside the handler — i.e. while the dispatch loop is blocked — it
///    spawns `body` on a raw `tokio::task::spawn` task (mimicking
///    `run_stream_loop`) using a clone of the agent's `ConnectionTo<Client>`,
///    then awaits that task before responding (mimicking `agent.prompt` awaiting
///    its stream to drain).
///
/// How the agent's prompt handler relates to the dispatch loop.
///
/// This mirrors the two production wirings: the original (broken) one that
/// awaited `agent.prompt(...)` inline inside the `on_receive_request` callback,
/// and the fixed one that runs the turn off the loop via [`ConnectionTo::spawn`].
#[derive(Clone, Copy)]
enum PromptDispatch {
    /// Await the streaming work inline inside the handler, blocking the dispatch
    /// loop for the whole turn. Reproduces the production **deadlock**.
    BlockOnLoop,
    /// Run the streaming work via `cx.spawn`, returning from the handler
    /// immediately so the loop stays free to route nested responses. Mirrors the
    /// **fix** in `swissarmyhammer-agent::dispatch_claude_request`.
    SpawnOffLoop,
}
/// The handler's response carries the `body` outcome JSON-encoded so the test
/// can recover it. A timeout on the prompt request turns the deadlock into a
/// fast, clear failure instead of an infinite run.
async fn elicitation_round_trip_during_prompt<F, Fut>(
    reply: FakeReply,
    dispatch: PromptDispatch,
    body: F,
) -> (ElicitationOutcome, Vec<UntypedMessage>)
where
    F: FnOnce(ConnectionTo<Client>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ElicitationOutcome> + Send + 'static,
{
    let recorded: Recorded = Arc::new(Mutex::new(Vec::new()));
    let (agent_side, client_side) = Channel::duplex();

    // The agent end: a real ACP Agent connection whose `on_receive_request`
    // handler models the production `prompt` dispatch — either blocking the
    // dispatch loop (the bug) or spawning the turn off the loop (the fix).
    let body_cell = Arc::new(Mutex::new(Some(body)));
    let agent_task = tokio::spawn(async move {
        let _ = Agent
            .builder()
            .name("elicitation-bridge-agent")
            .on_receive_request(
                move |_req: UntypedMessage,
                      responder: agent_client_protocol::Responder<JsonValue>,
                      cx: ConnectionTo<Client>| {
                    let body_cell = Arc::clone(&body_cell);
                    async move {
                        let body = body_cell
                            .lock()
                            .await
                            .take()
                            .expect("prompt handler invoked once");
                        // Mimic `query_stream`: the elicitation round-trip runs in
                        // its own raw `tokio::task::spawn` task on a clone of the
                        // stored client connection.
                        let conn_for_task = cx.clone();
                        let work = tokio::task::spawn(async move { body(conn_for_task).await });

                        match dispatch {
                            // Mimic the broken `agent.prompt(...).await`: block here
                            // (inside the dispatch-loop-owning handler) until the
                            // streaming work finishes. The loop is stuck so it can
                            // never route the nested elicitation response → deadlock.
                            PromptDispatch::BlockOnLoop => {
                                let outcome =
                                    work.await.expect("spawned elicitation task must not panic");
                                responder.respond_with_result(Ok(encode_outcome(&outcome)))
                            }
                            // Mimic the fix: run the turn off the dispatch loop via
                            // `cx.spawn`, returning from the handler immediately so
                            // the loop stays free to route the nested response.
                            PromptDispatch::SpawnOffLoop => cx.spawn(async move {
                                let outcome =
                                    work.await.expect("spawned elicitation task must not panic");
                                responder.respond_with_result(Ok(encode_outcome(&outcome)))
                            }),
                        }
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            .connect_to(agent_side)
            .await;
    });

    // The client end: answers elicitation/create and drives one prompt request.
    let client_recorded = Arc::clone(&recorded);
    let outcome = Client
        .builder()
        .name("test-client")
        .on_receive_request(
            move |req: UntypedMessage,
                  responder: agent_client_protocol::Responder<JsonValue>,
                  _cx: ConnectionTo<Agent>| {
                let recorded = Arc::clone(&client_recorded);
                let reply = reply.clone();
                async move {
                    recorded.lock().await.push(req.clone());
                    let result = if req.method() == "elicitation/create" {
                        reply.to_result()
                    } else {
                        Err(agent_client_protocol::Error::method_not_found())
                    };
                    responder.respond_with_result(result)
                }
            },
            agent_client_protocol::on_receive_request!(),
        )
        .connect_with(client_side, async move |cx: ConnectionTo<Agent>| {
            // Drive one "prompt" request and decode the encoded outcome.
            let prompt = UntypedMessage::new("prompt", json!({ "kind": "prompt" }))
                .expect("prompt message must encode");
            let value = cx.send_request(prompt).block_task().await?;
            Ok::<ElicitationOutcome, agent_client_protocol::Error>(decode_outcome(value))
        })
        .await
        .expect("client connect_with should succeed");

    agent_task.abort();
    let _ = agent_task.await;

    let seen = recorded.lock().await.clone();
    (outcome, seen)
}

/// Encode an [`ElicitationOutcome`] as JSON so it can ride back on the prompt
/// handler's response (the test driver decodes it with [`decode_outcome`]).
fn encode_outcome(outcome: &ElicitationOutcome) -> JsonValue {
    match outcome {
        ElicitationOutcome::Responded(response) => {
            json!({ "kind": "responded", "response": response })
        }
        ElicitationOutcome::Error(message) => {
            json!({ "kind": "error", "message": message })
        }
    }
}

/// Inverse of [`encode_outcome`].
fn decode_outcome(value: JsonValue) -> ElicitationOutcome {
    match value.get("kind").and_then(|k| k.as_str()) {
        Some("responded") => {
            let response: CreateElicitationResponse =
                serde_json::from_value(value["response"].clone())
                    .expect("encoded response must decode");
            ElicitationOutcome::Responded(response)
        }
        Some("error") => {
            ElicitationOutcome::Error(value["message"].as_str().unwrap_or_default().to_string())
        }
        other => panic!("unexpected encoded outcome kind: {other:?}"),
    }
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

/// The fix, exercised end-to-end: the elicitation round-trip is issued from a
/// raw `tokio::task::spawn` task (mimicking `run_stream_loop`) **while the
/// prompt turn runs off the dispatch loop via `cx.spawn`** (mimicking the fixed
/// `dispatch_claude_request`). Because the loop is free, it routes the
/// `elicitation/create` response back to the spawned task and the accept value
/// round-trips well within the timeout.
///
/// This is the regression test for the production bug: if someone reverts the
/// prompt dispatch to await `agent.prompt(...)` inline on the loop, the routed
/// response can no longer be delivered (see
/// [`prompt_handler_blocking_the_loop_deadlocks_the_elicitation`]) and this test
/// would time out. The timeout makes such a regression a *clear, fast* failure.
#[tokio::test]
async fn accept_round_trips_from_raw_spawned_task_during_prompt() {
    let request = sample_cli_elicitation();
    let request_for_call = request.clone();

    let fut = elicitation_round_trip_during_prompt(
        FakeReply::Accept(json!({ "name": "Alice" })),
        PromptDispatch::SpawnOffLoop,
        move |conn| async move {
            let handler = handler_with_capability(conn);
            handler
                .handle_elicitation(&request_for_call, SessionId::new("sess_spawned"))
                .await
        },
    );

    let (outcome, seen) = tokio::time::timeout(std::time::Duration::from_secs(10), fut)
        .await
        .expect(
            "elicitation round-trip from a raw spawned task must complete while the prompt turn \
             runs off the dispatch loop; a timeout here means the response was not routed back \
             (a regression of the production fix)",
        );

    let acp_response = expect_responded(outcome);

    assert_eq!(
        seen.len(),
        1,
        "expected exactly one elicitation/create request, saw {}",
        seen.len()
    );
    assert_eq!(seen[0].method(), "elicitation/create");
    assert_eq!(seen[0].params()["sessionId"], "sess_spawned");

    match &acp_response.action {
        ElicitationAction::Accept(accept) => {
            let content = accept.content.as_ref().expect("accept must carry content");
            let value = content.get("name").expect("name key must be present");
            assert_eq!(
                serde_json::to_value(value).unwrap(),
                json!("Alice"),
                "accepted content must round-trip back to the spawned task"
            );
        }
        other => panic!("expected accept, got {other:?}"),
    }
}

/// Documents the root cause: when the prompt turn is awaited *inline on the
/// dispatch loop* (the original, broken wiring), the loop is blocked for the
/// whole turn and can never route the nested `elicitation/create` response back
/// to the `block_task()` awaiter in the spawned streaming task. The round-trip
/// therefore never completes.
///
/// We assert exactly that: the call does **not** finish within a short window.
/// A short timeout is the *expected, asserted* outcome here — it pins the
/// failure mode so the contrast with
/// [`accept_round_trips_from_raw_spawned_task_during_prompt`] (the fix) is
/// explicit and self-documenting.
#[tokio::test]
async fn prompt_handler_blocking_the_loop_deadlocks_the_elicitation() {
    let request = sample_cli_elicitation();
    let request_for_call = request.clone();

    let fut = elicitation_round_trip_during_prompt(
        FakeReply::Accept(json!({ "name": "Alice" })),
        PromptDispatch::BlockOnLoop,
        move |conn| async move {
            let handler = handler_with_capability(conn);
            handler
                .handle_elicitation(&request_for_call, SessionId::new("sess_blocked"))
                .await
        },
    );

    let result = tokio::time::timeout(std::time::Duration::from_secs(2), fut).await;

    assert!(
        result.is_err(),
        "blocking the dispatch loop with the prompt turn MUST deadlock the elicitation \
         round-trip; if this completed, the deadlock is gone and this test should be removed"
    );
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
